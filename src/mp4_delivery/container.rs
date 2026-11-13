//! Independent closed ISO-BMFF observer. No FFmpeg claims or codec engine.
use super::*;
use anyhow::{Context,bail};
use std::collections::BTreeMap;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Time {pub numerator:i64,pub denominator:u64}
impl Ord for Time {fn cmp(&self,other:&Self)->std::cmp::Ordering{(i128::from(self.numerator)*i128::from(other.denominator)).cmp(&(i128::from(other.numerator)*i128::from(self.denominator)))}}
impl PartialOrd for Time {fn partial_cmp(&self,other:&Self)->Option<std::cmp::Ordering>{Some(self.cmp(other))}}
impl Time {
    pub(crate) fn new(n:i128,d:u64)->Result<Self>{ensure!(d>0,"positive clock denominator required");let mut a=n.unsigned_abs();let mut b=u128::from(d);while b!=0{let r=a%b;a=b;b=r;}let g=a.max(1);Ok(Self {numerator:i64::try_from(n/i128::try_from(g)?)?,denominator:u64::try_from(u128::from(d)/g)?})}
    pub(crate) fn ticks(n:i64,scale:u32)->Result<Self>{Self::new(i128::from(n),u64::from(scale))}
    pub(crate) fn add(self,other:Self)->Result<Self>{Self::new(i128::from(self.numerator)*i128::from(other.denominator)+i128::from(other.numerator)*i128::from(self.denominator),self.denominator.checked_mul(other.denominator).context("clock product overflow")?)}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackSummary {
    pub kind:String,pub timescale:u32,pub sample_count:u32,pub width:u16,pub height:u16,pub channels:u16,pub sample_rate:u32,
    pub codec_configuration:Vec<u8>,pub payload_bytes:u64,pub payload_digest:EvidenceDigest,pub presentation_digest:EvidenceDigest,
    pub frame_period:Option<Time>,pub presentation_start:Time,pub presentation_end:Time,pub edit_start:Time,pub edit_duration:Time,
    pub auxiliary:BTreeMap<String,EvidenceDigest>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {pub schema:String,pub movie_timescale:u32,pub video:TrackSummary,pub audio:TrackSummary,pub metadata:BTreeMap<String,Vec<u8>>,pub qualification:String}
#[derive(Debug,Clone)]
pub(crate) struct Sample {pub offset:u64,pub size:u32,pub pts:Time,pub duration:Time}
pub(crate) struct Parsed {pub summary:Summary}
#[derive(Clone,Copy)]struct Atom<'a>{kind:[u8;4],data:&'a[u8]}
fn slice(v:&[u8],at:usize,n:usize)->Result<&[u8]>{v.get(at..at.checked_add(n).context("MP4 range overflow")?).context("truncated MP4 field")}
fn u16_at(v:&[u8],at:usize)->Result<u16>{Ok(u16::from_be_bytes(slice(v,at,2)?.try_into()?))}
fn u32_at(v:&[u8],at:usize)->Result<u32>{Ok(u32::from_be_bytes(slice(v,at,4)?.try_into()?))}
fn u64_at(v:&[u8],at:usize)->Result<u64>{Ok(u64::from_be_bytes(slice(v,at,8)?.try_into()?))}
fn atoms(v:&[u8])->Result<Vec<Atom<'_>>>{let mut at=0;let mut result=vec![];while at<v.len(){ensure!(result.len()<128,"MP4 child atom count exceeded");let size=u32_at(v,at)?;let kind=slice(v,at+4,4)?.try_into()?;let (size,header)=if size==1{(usize::try_from(u64_at(v,at+8)?)?,16)}else{(size as usize,8)};ensure!(size>=header,"zero/short MP4 atom refused");result.push(Atom {kind,data:slice(v,at+header,size-header)?});at=at.checked_add(size).context("atom size overflow")?;}Ok(result)}
fn one<'a>(a:&[Atom<'a>],kind:&[u8;4])->Result<Atom<'a>>{let found=a.iter().filter(|a|&a.kind==kind).collect::<Vec<_>>();ensure!(found.len()==1,"required MP4 atom missing/duplicated");Ok(*found[0])}
fn allowed(a:&[Atom<'_>],kinds:&[[u8;4]])->Result<()>{ensure!(a.iter().all(|v|kinds.contains(&v.kind)),"unsupported MP4 atom/feature");Ok(())}
fn full(v:&[u8])->Result<()> {ensure!(u32_at(v,0)?==0,"unsupported MP4 full-box version/flags");Ok(())}
fn clock(v:&[u8])->Result<(u32,u64)> {let version=*v.first().context("missing MP4 clock")?;let (scale,duration)=match version{0=>(u32_at(v,12)?,u64::from(u32_at(v,16)?)),1=>(u32_at(v,20)?,u64_at(v,24)?),_=>bail!("unsupported MP4 clock version")};ensure!((1..=1_000_000_000).contains(&scale)&&duration>0&&duration<=u64::from(scale)*901,"MP4 clock exceeds bounded duration");Ok((scale,duration))}
fn table(v:&[u8],width:usize,maximum:usize)->Result<&[u8]>{full(v)?;let count=u32_at(v,4)? as usize;ensure!(count<=maximum&&v.len()==8+count.checked_mul(width).context("sample table overflow")?,"bounded complete MP4 sample table required");Ok(&v[8..])}
fn repeated(v:&[u8],signed:bool,maximum:usize)->Result<Vec<i64>>{
    ensure!(v.len()>=8&&v[1..4]==[0,0,0]&&matches!(v[0],0|1)&&(!signed||v[0]<=1),"unsupported time table");
    let count=u32_at(v,4)? as usize;ensure!(count<=maximum&&v.len()==8+count*8,"bounded complete time table required");let mut result=vec![];
    for row in v[8..].chunks_exact(8){let n=u32_at(row,0)? as usize;ensure!(n>0&&result.len().checked_add(n).is_some_and(|n|n<=maximum),"sample-time count exceeds bound");let value=if signed&&v[0]==1{i64::from(i32::from_be_bytes(slice(row,4,4)?.try_into()?))}else{i64::from(u32_at(row,4)?)};result.extend(std::iter::repeat_n(value,n));}Ok(result)
}
fn edits(trak:&[Atom<'_>],movie:u32,track:u32,duration:u64)->Result<(Time,Time,i64)> {
    let selected=trak.iter().filter(|a|a.kind==*b"edts").collect::<Vec<_>>();ensure!(selected.len()<=1,"duplicate edit list");
    if selected.is_empty(){return Ok((Time::ticks(0,movie)?,Time::new(i128::from(duration),u64::from(track))?,0));}
    let children=atoms(selected[0].data)?;allowed(&children,&[*b"elst"])?;let v=one(&children,b"elst")?.data;
    ensure!(v.len()>=8&&v[1..4]==[0,0,0]&&matches!(v[0],0|1),"unsupported edit version/flags");
    let width=if v[0]==0{12}else{20};let count=u32_at(v,4)? as usize;ensure!((1..=2).contains(&count)&&v.len()==8+count*width,"one media edit plus optional leading empty edit required");
    let mut leading=0;let mut media=None;
    for (index,row) in v[8..].chunks_exact(width).enumerate(){let (len,time,rate)=if width==12{(u64::from(u32_at(row,0)?),i64::from(i32::from_be_bytes(slice(row,4,4)?.try_into()?)),u32_at(row,8)?)}else{(u64_at(row,0)?,i64::from_be_bytes(slice(row,8,8)?.try_into()?),u32_at(row,16)?)};
        ensure!(len>0&&len<=u64::from(movie)*900&&rate==0x00010000,"unsupported empty/rate/duration edit");
        if time== -1 {ensure!(index==0&&count==2,"only one leading empty edit supported");leading=len;}else{ensure!(time>=0&&media.is_none(),"negative/repeated media edit refused");media=Some((len,time));}}
    let (len,time)=media.context("media edit absent")?;Ok((Time::new(i128::from(leading),u64::from(movie))?,Time::new(i128::from(len),u64::from(movie))?,time))
}
fn descriptor(v:&[u8],tag:u8)->Result<&[u8]>{ensure!(v.first()==Some(&tag),"unexpected MPEG4 descriptor");let mut len=0usize;let mut at=1;loop{let b=*v.get(at).context("truncated descriptor length")?;at+=1;ensure!(at<=5,"descriptor length overflow");len=len.checked_mul(128).and_then(|n|n.checked_add(usize::from(b&127))).context("descriptor overflow")?;if b&128==0{break;}}slice(v,at,len)}
fn aac(v:&[u8],channels:u16,rate:u32)->Result<Vec<u8>>{full(v)?;let es=descriptor(&v[4..],3)?;ensure!(es.len()>=3&&es[2]==0,"complex MPEG4 ES dependency refused");let decoder=descriptor(&es[3..],4)?;ensure!(decoder.len()>=13&&decoder[0]==0x40&&(decoder[1]>>2)==5,"AAC elementary stream required");let asc=descriptor(&decoder[13..],5)?;ensure!((2..=64).contains(&asc.len()),"bounded AAC AudioSpecificConfig required");let bits=u16_at(asc,0)?;let frequency=(bits>>7)&15;ensure!((bits>>11)==2&&((bits>>3)&15)==channels&&bits&4==0&&match frequency{3=>rate==48000,4=>rate==44100,_=>false},"only complete 1024-sample mono/stereo AAC-LC at 44.1/48 kHz supported");Ok(asc.to_vec())}
fn metadata(moov:&[Atom<'_>])->Result<BTreeMap<String,Vec<u8>>>{
    let mut result=BTreeMap::new();let udta=moov.iter().filter(|a|a.kind==*b"udta").collect::<Vec<_>>();ensure!(udta.len()<=1,"duplicate user metadata");if udta.is_empty(){return Ok(result);}
    let a=atoms(udta[0].data)?;allowed(&a,&[*b"meta"])?;if a.is_empty(){return Ok(result);}let meta=one(&a,b"meta")?.data;full(meta)?;let a=atoms(&meta[4..])?;allowed(&a,&[*b"hdlr",*b"ilst"])?;
    if a.iter().any(|a|a.kind==*b"ilst"){for tag in atoms(one(&a,b"ilst")?.data)?{ensure!([*b"\xa9nam",*b"\xa9ART",*b"\xa9alb",*b"\xa9cmt",*b"\xa9day",*b"desc",*b"cprt",*b"\xa9too"].contains(&tag.kind),"unsupported descriptive/attached metadata");let data=atoms(tag.data)?;let value=one(&data,b"data")?.data;allowed(&data,&[*b"data"])?;ensure!(value.len()<=65536&&value.len()>=8&&u32_at(value,0)?==1,"bounded UTF8 metadata required");std::str::from_utf8(&value[8..])?;if tag.kind!=*b"\xa9too"{let key=tag.kind.iter().map(|b|format!("{b:02x}")).collect::<String>();ensure!(result.insert(key,value.to_vec()).is_none(),"duplicate metadata field");}}}Ok(result)
}
fn track(trak:Atom<'_>,movie:u32,read:&impl Fn(u64,usize)->Result<Vec<u8>>,mdat:(u64,u64))->Result<(TrackSummary,Vec<Sample>)>{
    let t=atoms(trak.data)?;allowed(&t,&[*b"tkhd",*b"edts",*b"mdia"])?;let tkhd=one(&t,b"tkhd")?.data;let matrix=match tkhd.first(){Some(0)=>40,Some(1)=>52,_=>bail!("unsupported track header")};
    let expected=[0x00010000u32,0,0,0,0x00010000,0,0,0,0x40000000];for (i,n) in expected.iter().enumerate(){ensure!(u32_at(tkhd,matrix+i*4)?==*n,"rotation/shear/projective track unsupported");}
    let mdia=atoms(one(&t,b"mdia")?.data)?;allowed(&mdia,&[*b"mdhd",*b"hdlr",*b"minf"])?;let (scale,duration)=clock(one(&mdia,b"mdhd")?.data)?;let handler=slice(one(&mdia,b"hdlr")?.data,8,4)?;let video=handler==b"vide";ensure!(video||handler==b"soun","only one video and one audio track supported");
    let m=atoms(one(&mdia,b"minf")?.data)?;allowed(&m,&[*b"vmhd",*b"smhd",*b"dinf",*b"stbl"])?;let d=atoms(one(&m,b"dinf")?.data)?;allowed(&d,&[*b"dref"])?;let dref=one(&d,b"dref")?.data;full(dref)?;ensure!(u32_at(dref,4)?==1,"one self-contained media reference required");let url=atoms(&dref[8..])?;let data=one(&url,b"url ")?.data;allowed(&url,&[*b"url "])?;ensure!(data==[0,0,0,1],"external data references refused");
    let s=atoms(one(&m,b"stbl")?.data)?;allowed(&s,&[*b"stsd",*b"stts",*b"ctts",*b"stss",*b"stsc",*b"stsz",*b"stco",*b"co64",*b"sdtp",*b"sgpd",*b"sbgp"])?;let stsd=one(&s,b"stsd")?.data;full(stsd)?;ensure!(u32_at(stsd,4)?==1,"one sample description required");let e=atoms(&stsd[8..])?;ensure!(e.len()==1&&e[0].kind==if video{*b"avc1"}else{*b"mp4a"},"AVC1 video and MPEG4 AAC required");let entry=e[0].data;ensure!(u16_at(entry,6)?==1,"sample entry data reference differs");
    let (width,height,channels,rate,config)=if video {
        ensure!(entry.len()>=78,"short AVC entry");let (w,h)=(u16_at(entry,24)?,u16_at(entry,26)?);ensure!((2..=4096).contains(&w)&&(2..=2160).contains(&h)&&w%2==0&&h%2==0,"bounded even video geometry required");
        let ext=atoms(&entry[78..])?;allowed(&ext,&[*b"avcC",*b"pasp",*b"colr",*b"btrt",*b"fiel"])?;let avc=one(&ext,b"avcC")?.data;ensure!((7..=65536).contains(&avc.len())&&avc[0]==1&&[66,77,100].contains(&avc[1])&&avc[4]&3==3,"bounded 8-bit AVC declaration required; native pixel verification remains required");
        let colr=one(&ext,b"colr")?.data;ensure!((colr.len()==10&&&colr[..4]==b"nclc"||colr.len()==11&&&colr[..4]==b"nclx"&&colr[10]==0)&&u16_at(colr,4)?==1&&u16_at(colr,6)?==1&&u16_at(colr,8)?==1,"explicit limited BT709 SDR container color required");
        for a in &ext{if a.kind==*b"pasp"{ensure!(a.data.len()==8&&u32_at(a.data,0)?>0&&u32_at(a.data,0)?==u32_at(a.data,4)?,"square pixels required");}if a.kind==*b"fiel"{ensure!(a.data==[1,0],"progressive field declaration required");}}
        (w,h,0,0,avc.to_vec())
    }else{ensure!(entry.len()>=28&&u16_at(entry,8)?==0,"version-0 AAC sample entry required");let c=u16_at(entry,16)?;let fixed=u32_at(entry,24)?;let r=fixed>>16;ensure!(matches!(c,1|2)&&fixed&65535==0&&matches!(r,44100|48000),"mono/stereo 44.1/48 kHz AAC required");let ext=atoms(&entry[28..])?;allowed(&ext,&[*b"esds",*b"btrt"])?;(0,0,c,r,aac(one(&ext,b"esds")?.data,c,r)?)};
    let stsz=one(&s,b"stsz")?.data;full(stsz)?;let constant=u32_at(stsz,4)?;let count=u32_at(stsz,8)? as usize;let maximum=if video{MAX_FRAMES}else{MAX_AUDIO_PACKETS};ensure!(count>0&&count<=maximum&&stsz.len()==12+if constant==0{count*4}else{0},"bounded complete sample sizes required");let sizes=(0..count).map(|i|if constant==0{u32_at(stsz,12+i*4)}else{Ok(constant)}).collect::<Result<Vec<_>>>()?;
    ensure!(sizes.iter().all(|n|*n>0&&*n<=8_388_608),"sample byte bound exceeded");
    let durations=repeated(one(&s,b"stts")?.data,false,maximum)?;ensure!(durations.len()==count&&durations.iter().all(|n|*n>0),"complete positive sample durations required");
    let ctts=s.iter().filter(|a|a.kind==*b"ctts").collect::<Vec<_>>();ensure!(ctts.len()<=1,"duplicate composition offsets");let offsets=if ctts.is_empty(){vec![0;count]}else{repeated(ctts[0].data,true,maximum)?};ensure!(offsets.len()==count,"composition count differs");
    let stco=s.iter().filter(|a|a.kind==*b"stco"||a.kind==*b"co64").collect::<Vec<_>>();ensure!(stco.len()==1,"one chunk offset table required");let wide=stco[0].kind==*b"co64";let chunk_table=table(stco[0].data,if wide{8}else{4},maximum)?;let chunks=chunk_table.chunks_exact(if wide{8}else{4}).map(|r|if wide{u64_at(r,0)}else{Ok(u64::from(u32_at(r,0)?))}).collect::<Result<Vec<_>>>()?;
    let map=table(one(&s,b"stsc")?.data,12,maximum)?.chunks_exact(12).map(|r|Ok((u32_at(r,0)?,u32_at(r,4)?,u32_at(r,8)?))).collect::<Result<Vec<_>>>()?;ensure!(!map.is_empty()&&map[0].0==1&&map.windows(2).all(|p|p[0].0<p[1].0)&&map.iter().all(|v|v.1>0&&v.2==1&&v.0 as usize<=chunks.len()),"unsupported chunk/sample mapping");
    let (edit_start,edit_duration,media_time)=edits(&t,movie,scale,duration)?;let mut samples=vec![];let mut index=0;let mut dts=0i64;let mut row=0;
    for (chunk,position) in chunks.iter().enumerate(){while row+1<map.len()&&map[row+1].0 as usize<=chunk+1{row+=1;}let mut at=*position;for _ in 0..map[row].1{ensure!(index<count,"chunk exceeds sample sizes");let size=sizes[index];ensure!(at>=mdat.0&&at.checked_add(u64::from(size)).is_some_and(|end|end<=mdat.1),"sample outside owned mdat");let pts=Time::ticks(dts.checked_add(offsets[index]).and_then(|n|n.checked_sub(media_time)).context("PTS overflow")?,scale)?.add(edit_start)?;samples.push(Sample {offset:at,size,pts,duration:Time::ticks(durations[index],scale)?});dts=dts.checked_add(durations[index]).context("DTS overflow")?;at+=u64::from(size);index+=1;}}
    ensure!(index==count&&dts as u64==duration,"complete sample table duration/count differs");
    let mut timeline=samples.iter().map(|s|(s.pts,s.duration)).collect::<Vec<_>>();timeline.sort_by(|a,b|(i128::from(a.0.numerator)*i128::from(b.0.denominator)).cmp(&(i128::from(b.0.numerator)*i128::from(a.0.denominator))));
    let frame_period=if video{let delta=Time::ticks(durations[0],scale)?;ensure!(durations.iter().all(|n|*n==durations[0])&&timeline.windows(2).all(|w|w[0].0.add(delta).ok()==Some(w[1].0))&&delta.numerator>0&&i128::from(delta.numerator)*120>=i128::from(delta.denominator)&&delta.numerator as u64<=delta.denominator,"constant progressive frame clock 1..120fps required");Some(delta)}else{ensure!(offsets.iter().all(|n|*n==0),"AAC composition reordering unsupported");None};
    let mut payload=blake3::Hasher::new();let mut bytes=0u64;for sample in &samples{let data=read(sample.offset,sample.size as usize)?;payload.update(&sample.size.to_be_bytes());payload.update(&data);bytes=bytes.checked_add(u64::from(sample.size)).context("sample byte sum overflow")?;}
    let mut auxiliary=BTreeMap::new();for a in &s{if !video&&[*b"sgpd",*b"sbgp",*b"sdtp"].contains(&a.kind){let key=String::from_utf8(a.kind.to_vec())?;ensure!(auxiliary.insert(key,hash(a.data)).is_none(),"duplicate auxiliary audio table");}}
    if let Some(sync)=s.iter().find(|a|a.kind==*b"stss"){let values=table(sync.data,4,maximum)?.chunks_exact(4).map(|r|u32_at(r,0)).collect::<Result<Vec<_>>>()?;ensure!(values.iter().all(|n|*n>0&&*n<=count as u32)&&values.windows(2).all(|w|w[0]<w[1]),"invalid sync sample table");}
    let first=timeline[0].0;let last=timeline.last().context("sample timeline empty")?;let end=last.0.add(last.1)?;
    Ok((TrackSummary {kind:if video{"video_h264"}else{"audio_aac_lc"}.into(),timescale:scale,sample_count:count as u32,width,height,channels,sample_rate:rate,codec_configuration:config,payload_bytes:bytes,payload_digest:EvidenceDigest {algorithm:"blake3-256".into(),value:payload.finalize().to_hex().into()},presentation_digest:digest(&timeline)?,frame_period,presentation_start:first,presentation_end:end,edit_start,edit_duration,auxiliary},samples))
}
pub(crate) fn observe(file:&BoundedFileObservation,guard:&impl Fn()->Result<()>)->Result<Parsed>{
    let read=|offset,count|file.range(offset,count,guard);let length=file.snapshot().logical_bytes;let mut at=0u64;let mut movie=None;let mut mdat=None;let mut brand=false;let mut count=0;
    while at<length {guard()?;count+=1;ensure!(count<=64,"too many top-level MP4 atoms");let header=read(at,8)?;let short=u32_at(&header,0)?;let kind=slice(&header,4,4)?;let (size,n)=if short==1{(u64_at(&read(at+8,8)?,0)?,16u64)}else{(u64::from(short),8)};ensure!(size>=n&&at.checked_add(size).is_some_and(|n|n<=length),"truncated/unbounded top-level MP4 atom");
        match kind {b"ftyp"=>{ensure!(!brand&&size<=4096,"duplicate/large MP4 brand");let f=read(at+n,(size-n) as usize)?;ensure!(f.len()>=8&&f.len()%4==0&&f.chunks_exact(4).enumerate().filter(|(i,_)|*i!=1).all(|(_,b)|[b"isom".as_slice(),b"iso2",b"iso6",b"mp41",b"mp42",b"avc1"].contains(&b)),"unsupported MP4 brand");brand=true;},b"moov"=>{ensure!(movie.is_none()&&size-n<=METADATA_BYTES as u64,"bounded single moov required");movie=Some(read(at+n,(size-n) as usize)?);},b"mdat"=>{ensure!(mdat.is_none()&&size>n,"single finite mdat required");mdat=Some((at+n,at+size));},b"free"|b"skip"|b"wide"=>{},_=>bail!("unsupported fragmented/external MP4 top-level feature")};at+=size;
    }
    ensure!(brand,"MP4 brand missing");let mdat=mdat.context("MP4 media missing")?;let data=movie.context("MP4 movie missing")?;let m=atoms(&data)?;allowed(&m,&[*b"mvhd",*b"trak",*b"udta"])?;let (scale,_)=clock(one(&m,b"mvhd")?.data)?;
    let tracks=m.iter().filter(|a|a.kind==*b"trak").copied().collect::<Vec<_>>();ensure!(tracks.len()==2,"exactly one H264 and one AAC track required");let mut parsed=tracks.into_iter().map(|t|track(t,scale,&read,mdat)).collect::<Result<Vec<_>>>()?;
    let video=parsed.iter().position(|t|t.0.kind=="video_h264").context("H264 video absent")?;let (v,vs)=parsed.remove(video);ensure!(parsed.len()==1&&parsed[0].0.kind=="audio_aac_lc","one AAC track required");let (a,asamples)=parsed.remove(0);
    ensure!([&v,&a].iter().all(|t|t.edit_duration.numerator>0&&i128::from(t.edit_duration.numerator)<=i128::from(t.edit_duration.denominator)*900),"supported complete edit duration exceeds fifteen minutes");
    let mut ranges=vs.iter().chain(&asamples).map(|s|(s.offset,s.offset+u64::from(s.size))).collect::<Vec<_>>();ranges.sort_unstable();ensure!(ranges.windows(2).all(|w|w[0].1<=w[1].0),"overlapping media sample extents refused");
    Ok(Parsed {summary:Summary {schema:"optiflow.mp4-container-observation.v1".into(),movie_timescale:scale,video:v,audio:a,metadata:metadata(&m)?,qualification:"structural_observation_only; native_probe_decode_and_human_review_required".into()}})
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn exact_clocks_reduce_and_do_not_use_float_rounding(){assert_eq!(Time::new(1001,30000).unwrap().add(Time::new(1001,30000).unwrap()).unwrap(),Time::new(1001,15000).unwrap());assert!(Time::new(1,0).is_err());assert_eq!(Time::new(-1024,48000).unwrap(),Time::new(-8,375).unwrap());}
    #[test]fn malformed_atom_sizes_and_oversized_expansion_are_refused(){assert!(atoms(&[0,0,0,0,b'm',b'o',b'o',b'v']).is_err());assert!(atoms(&[0,0,0,40,b'm',b'o',b'o',b'v']).is_err());let v=[0,0,0,0,0,0,0,1,255,255,255,255,0,0,0,1];assert!(repeated(&v,false,MAX_FRAMES).is_err());}
}
