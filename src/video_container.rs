//! Independent, closed EBML view for the first FFV1 stream-copy subset.
//! Unknown elements, CRC elements, lacing, extra streams, attachments, chapters
//! and descriptive tags refuse. This is not a general Matroska validator.
use std::collections::{BTreeMap,BTreeSet};
use anyhow::{Result,ensure,bail,Context};
use serde::{Serialize,Deserialize};
use crate::domain::EvidenceDigest;
use crate::video_profile::Profile;

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet { pub pts_milliseconds: u64, pub keyframe: bool, pub bytes: u64, pub digest: EvidenceDigest }
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerObservation {
    pub profile: String,
    pub width: u32,
    pub height: u32,
    pub track_contract: BTreeMap<String,String>,
    pub codec_private_digest: EvidenceDigest,
    pub packets: Vec<Packet>,
    pub technical_tag_count: u64,
    pub void_bytes: u64,
}
struct Element<'a> { id:u64, data:&'a [u8] }
fn vint(bytes: &[u8],id: bool) -> Result<(u64,usize,bool)> {
    let first = *bytes.first().context("truncated EBML integer")?;
    ensure!(first != 0,"invalid EBML integer");
    let width = first.leading_zeros() as usize+1;
    ensure!(width <= (if id {4} else {8}) && bytes.len() >= width,"unsupported or truncated EBML integer");
    let mut value = if id {u64::from(first)} else {u64::from(first & ((0xff_u16 >> width) as u8))};
    for byte in &bytes[1..width] { value = (value<<8)|u64::from(*byte); }
    let unknown = !id && value == (1_u64 << (7*width))-1;
    Ok((value,width,unknown))
}
fn elements(mut bytes: &[u8],allow_segment: bool) -> Result<Vec<Element<'_>>> {
    let mut values = Vec::new();
    while !bytes.is_empty() {
        ensure!(values.len() < 4096,"EBML sibling count exceeds bound");
        let (id,a,_) = vint(bytes,true)?;
        let (size,b,unknown) = vint(&bytes[a..],false)?;
        let data = &bytes[a+b..];
        let length = if unknown {ensure!(allow_segment && id == 0x18538067,"unknown EBML length outside top segment");data.len()}
            else {usize::try_from(size).context("EBML size overflow")?};
        ensure!(length <= data.len(),"EBML element extends beyond its complete bytes");
        values.push(Element {id,data:&data[..length]}); bytes = &data[length..];
    }
    Ok(values)
}
fn uint(bytes: &[u8]) -> Result<u64> {
    ensure!(!bytes.is_empty() && bytes.len() <= 8,"unsupported EBML unsigned width");
    Ok(bytes.iter().fold(0,|value,byte| (value<<8)|u64::from(*byte)))
}
fn digest(bytes: &[u8]) -> EvidenceDigest { EvidenceDigest {algorithm:"blake3-256".to_owned(),value:blake3::hash(bytes).to_hex().to_string()} }
fn text(bytes: &[u8]) -> Result<&str> { ensure!(bytes.len() <= 1024,"EBML text exceeds bound");let value=std::str::from_utf8(bytes)?;ensure!(!value.chars().any(char::is_control),"unsupported control in EBML text");Ok(value) }
fn unique<'a>(values: &'a [Element<'a>],id: u64) -> Result<&'a [u8]> {
    let mut matches=values.iter().filter(|v| v.id == id);let value=matches.next().context("required EBML element missing")?;
    ensure!(matches.next().is_none(),"duplicate required EBML element");Ok(value.data)
}
fn no_duplicates(values: &[Element<'_>]) -> Result<()> {
    let mut seen=BTreeSet::new();for v in values {if v.id != 0xec {ensure!(seen.insert(v.id),"duplicate singleton EBML field");}}Ok(())
}

/// Acquire complete bounded structure and elementary packets from actual bytes.
pub fn inspect(bytes: &[u8]) -> Result<ContainerObservation> {inspect_profile(bytes,Profile::TinyV1)}
/// Explicit v2 structural inspection, not native decode or candidate acceptance.
pub fn inspect_v2(bytes:&[u8])->Result<ContainerObservation>{inspect_profile(bytes,Profile::BoundedV2)}
pub(crate) fn inspect_profile(bytes:&[u8],profile:Profile)->Result<ContainerObservation> {
    ensure!(!bytes.is_empty() && bytes.len() as u64 <= profile.input_bytes(),"Matroska input exceeds closed profile byte bound");
    let top = elements(bytes,true)?;
    ensure!(top.len() == 2 && top[0].id == 0x1a45dfa3 && top[1].id == 0x18538067,"expected one EBML header and one complete segment");
    let header=elements(top[0].data,false)?;no_duplicates(&header)?;
    ensure!(text(unique(&header,0x4282)?)? == "matroska","unsupported EBML document type");
    for v in &header {match v.id {
        0x4282=>(),0x4286|0x42f7=>ensure!(uint(v.data)? == 1,"unsupported EBML read/version"),
        0x42f2=>ensure!(uint(v.data)? == 4,"unsupported EBML ID width"),0x42f3=>ensure!(uint(v.data)? == 8,"unsupported EBML size width"),
        0x4287=>ensure!(uint(v.data)? <= 4,"unsupported Matroska version"),0x4285=>ensure!(uint(v.data)? <= 4,"unsupported Matroska read version"),
        _=>bail!("unsupported EBML header element"),}}
    let segment = elements(top[1].data,false)?;
    let info=elements(unique(&segment,0x1549a966)?,false)?;no_duplicates(&info)?;
    for v in &info {match v.id {
        0x2ad7b1=>ensure!(uint(v.data)? == 1_000_000,"unsupported Matroska timestamp scale"),
        0x4d80|0x5741=>{text(v.data)?;},0x73a4=>ensure!(v.data.len() == 16,"invalid technical segment UID"),
        0x4489=>{let value=match v.data.len(){4=>f64::from(f32::from_be_bytes(v.data.try_into()?)),8=>f64::from_be_bytes(v.data.try_into()?),_=>bail!("invalid duration width")};ensure!(value.is_finite() && value > 0.0 && value <= (profile.frames() as u64*40) as f64,"invalid technical duration");},
        0xec=>(),_=>bail!("unsupported/private Matroska info field"),}}
    let tracks=elements(unique(&segment,0x1654ae6b)?,false)?;
    ensure!(tracks.len() == 1 && tracks[0].id == 0xae,"one video track required");
    let track=elements(tracks[0].data,false)?;no_duplicates(&track)?;
    ensure!(uint(unique(&track,0xd7)?)? == 1 && uint(unique(&track,0x83)?)? == 1,"unsupported track identity/type");
    ensure!(uint(unique(&track,0x23e383)?)? == 40_000_000,"exact 25 fps track default duration required");
    let codec=text(unique(&track,0x86)?)?;
    ensure!(matches!(codec,"V_MS/VFW/FOURCC"|"V_FFV1"),"unsupported track codec");
    let codec_private=unique(&track,0x63a2)?;
    ensure!(!codec_private.is_empty() && codec_private.len() <= 65_536,"unsupported FFV1 codec private data");
    if codec == "V_MS/VFW/FOURCC" {ensure!(codec_private.len() >= 40 && &codec_private[16..20] == b"FFV1","FFV1 FOURCC required");}
    let track_uid=uint(unique(&track,0x73c5)?)?;
    let mut contract=BTreeMap::from([("enabled".to_owned(),"1".to_owned()),("default".to_owned(),"1".to_owned()),("forced".to_owned(),"0".to_owned()),("lacing".to_owned(),"1".to_owned()),("language".to_owned(),"eng".to_owned())]);
    for v in &track {match v.id {
        0xd7|0x83|0x23e383|0x86|0x63a2|0x73c5|0xe0=>(),
        0xb9|0x88|0x55aa|0x9c=>{let n=uint(v.data)?;ensure!(n<=1,"invalid track flag");let key=match v.id{0xb9=>"enabled",0x88=>"default",0x55aa=>"forced",_=>"lacing"};contract.insert(key.to_owned(),n.to_string());},
        0x22b59c=>{let language=text(v.data)?;ensure!(matches!(language,"und"|"eng"),"non-default language outside first subset");contract.insert("language".to_owned(),language.to_owned());},
        0xec=>(),_=>bail!("unsupported/private track element"),}}
    ensure!(contract.get("enabled").map(String::as_str)==Some("1") && contract.get("forced").map(String::as_str)==Some("0"),"disabled/forced track outside closed subset");
    let video=elements(unique(&track,0xe0)?,false)?;no_duplicates(&video)?;
    let width=uint(unique(&video,0xb0)?)?;let height=uint(unique(&video,0xba)?)?;
    ensure!((2..=u64::from(profile.width())).contains(&width) && (2..=u64::from(profile.height())).contains(&height) && width%2==0 && height%2==0,"unsupported closed-profile geometry");
    let mut colors=false;
    for v in &video {match v.id {
        0xb0|0xba=>(),
        0x54b0=>ensure!(uint(v.data)? == width,"display width changes aspect"),0x54ba=>ensure!(uint(v.data)? == height,"display height changes aspect"),
        0x54b2|0x54aa|0x54bb|0x54cc|0x54dd|0x53b8|0x53c0=>ensure!(uint(v.data)? == 0,"unsupported crop/stereo/alpha/display unit"),
        0x9a=>ensure!(matches!(uint(v.data)?,0|2),"interlaced video is unsupported"),
        0x55b0=>{colors=true;let values=elements(v.data,false)?;no_duplicates(&values)?;
            for color in &values {ensure!(matches!(color.id,0x55b1|0x55b2|0x55b3|0x55b4|0x55b7|0x55b8|0x55b9|0x55ba|0x55bb),"unknown HDR/color element");
                let n=uint(color.data)?;match color.id {0x55b1|0x55b9|0x55ba|0x55bb=>ensure!(n==1,"non BT.709 limited-range color refused"),0x55b2=>ensure!(n==8,"non 8-bit color refused"),0x55b3|0x55b4=>ensure!(n==1,"non 4:2:0 chroma refused"),_=>ensure!(n<=2,"unsupported chroma siting")};
                contract.insert(format!("color_{:x}",color.id),n.to_string());}
            for required in [0x55b1,0x55b9,0x55ba,0x55bb] {unique(&values,required)?;}
        },0xec=>(),_=>bail!("unknown video/color/transform element"),}}
    ensure!(colors,"explicit color contract required");
    let mut result=ContainerObservation {profile:profile.container_profile().to_owned(),width:width as u32,height:height as u32,
        track_contract:contract,codec_private_digest:digest(codec_private),packets:Vec::new(),technical_tag_count:0,void_bytes:0};
    let mut cluster_count=0;
    for v in &segment {match v.id {
        0x1549a966|0x1654ae6b=>(),
        0x1f43b675=>{cluster_count+=1;ensure!(cluster_count<=profile.frames(),"too many clusters");cluster(v.data,&mut result.packets,profile.frames())?;},
        0x114d9b74|0x1c53bb6b=>index_tree(v.id,v.data,0,top[1].data.len())?,
        0x1254c367=>{result.technical_tag_count+=technical_tags(v.data,track_uid)?;ensure!(result.technical_tag_count<=256,"aggregate technical tag count exceeds bound");},
        0xec=>result.void_bytes+=v.data.len() as u64,
        _=>bail!("unknown/CRC/attachment/chapter segment element refused"),}}
    ensure!(!result.packets.is_empty() && result.packets.len() <= profile.frames(),"unsupported complete video packet count");
    for (i,packet) in result.packets.iter().enumerate() {ensure!(packet.pts_milliseconds == i as u64*40,"nonzero/gapped/reordered packet clock refused");}
    Ok(result)
}

fn cluster(bytes: &[u8],out: &mut Vec<Packet>,maximum:usize) -> Result<()> {
    let values=elements(bytes,false)?;let time=uint(unique(&values,0xe7)?)?;
    for v in values {match v.id {
        0xe7|0xa7|0xab=>(),0xec=>(),0xa3=>block(v.data,time,None,out,maximum)?,
        0xa0=>{let group=elements(v.data,false)?;no_duplicates(&group)?;
            for field in &group {match field.id{0xa1=>(),0x9b=>ensure!(uint(field.data)? == 40,"unsupported block duration"),_=>bail!("block group extensions/reference dependencies refused")}}
            block(unique(&group,0xa1)?,time,Some(true),out,maximum)?;
        },_=>bail!("unknown/CRC cluster element refused"),}}
    Ok(())
}
fn block(bytes: &[u8],time: u64,group_keyframe: Option<bool>,out: &mut Vec<Packet>,maximum:usize) -> Result<()> {
    ensure!(out.len()<maximum,"packet count exceeds bound");let (track,width,unknown)=vint(bytes,false)?;
    ensure!(track==1 && !unknown && bytes.len()>width+3,"unsupported/truncated Matroska block");
    let relative=i16::from_be_bytes(bytes[width..width+2].try_into()?);let flags=bytes[width+2];
    ensure!(flags & 0x7f == 0 && (group_keyframe.is_none() || flags==0),"lacing/invisible/discardable/unknown block flags refused");
    let pts=i128::from(time)+i128::from(relative);ensure!(pts>=0,"negative video presentation timestamp refused");
    let data=&bytes[width+3..];
    out.push(Packet {pts_milliseconds:u64::try_from(pts)?,keyframe:group_keyframe.unwrap_or(flags&0x80!=0),bytes:data.len() as u64,digest:digest(data)});Ok(())
}
fn index_tree(id: u64,bytes: &[u8],depth: u8,length: usize) -> Result<()> {
    ensure!(depth<=4,"Matroska index nesting exceeds bound");
    for v in elements(bytes,false)? {match (id,v.id) {
        (0x114d9b74,0x4dbb)|(0x1c53bb6b,0xbb)|(0xbb,0xb7)=>index_tree(v.id,v.data,depth+1,length)?,
        (0x4dbb,0x53ab)=>{let (target,width,_)=vint(v.data,true)?;ensure!(width==v.data.len() && matches!(target,0x1549a966|0x1654ae6b|0x1f43b675|0x1c53bb6b|0x1254c367),"unsupported seek target");},
        (0x4dbb,0x53ac)|(0xb7,0xf1)|(0xb7,0xf0)=>ensure!(uint(v.data)? < length as u64,"index pointer outside segment"),
        (0xbb,0xb3)=>{uint(v.data)?;},(0xb7,0xf7)=>ensure!(uint(v.data)? == 1,"index references unselected track"),
        (0xb7,0xb2)|(0xb7,0x5378)=>{uint(v.data)?;},(_,0xec)=>(),_=>bail!("unknown/CRC Matroska index element"),}}
    Ok(())
}
fn technical_tags(bytes: &[u8],track_uid: u64) -> Result<u64> {
    let mut count=0;
    for tag in elements(bytes,false)? {ensure!(tag.id==0x7373,"unknown Matroska tag element");
        for v in elements(tag.data,false)? {match v.id {
            0x63c0=>{for target in elements(v.data,false)? {match target.id {
                0x63c5=>ensure!(matches!(uint(target.data)?,0) || uint(target.data)? == track_uid,"tag targets unrelated track"),
                0x68ca=>ensure!(uint(target.data)? == 50,"unsupported tag target type"),_=>bail!("unsupported tag target")}}},
            0x67c8=>{let fields=elements(v.data,false)?;no_duplicates(&fields)?;
                ensure!(matches!(text(unique(&fields,0x45a3)?)?,"ENCODER"|"DURATION"),"descriptive/unknown metadata tag refused");text(unique(&fields,0x4487)?)?;
                for field in fields {match field.id {0x45a3|0x4487=>(),0x447a=>ensure!(text(field.data)? == "und","unsupported tag language"),0x4484=>ensure!(uint(field.data)?==1,"nondefault tag refused"),_=>bail!("nested/binary/unknown tag refused")}}
                count+=1;ensure!(count<=256,"too many technical metadata tags");
            },_=>bail!("unknown Matroska tag scope"),}}
    }Ok(count)
}

pub fn compare_stream_copy(source: &ContainerObservation,candidate: &ContainerObservation) -> Result<()> {
    ensure!(source.profile == candidate.profile && source.width == candidate.width && source.height == candidate.height
        && source.track_contract == candidate.track_contract && source.codec_private_digest == candidate.codec_private_digest
        && source.packets == candidate.packets,"candidate changed supported codec/color/aspect/stream/packet/timing semantics");Ok(())
}
