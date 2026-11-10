//! Safe host integration around a supplied descriptor-only property backend.
//! No native backend is adopted here; default production dispatch stays refused.
use std::fs::File;
use anyhow::{Result,ensure,bail,Context};
use serde::{Serialize,Deserialize};
use super::property_evidence::{self,MacApfsCrossCopyPropertiesV1,ReviewedProperties,Timestamp};
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendIdentity {pub schema:String,pub implementation_id:String,pub source_sha256:String}
impl BackendIdentity {pub fn validate(&self)->Result<()>{ensure!(self.schema=="optiflow.descriptor-property-backend.v1"&&!self.implementation_id.is_empty()&&self.implementation_id.len()<=128&&self.implementation_id.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-:/".contains(&c))&&self.source_sha256.len()==64&&self.source_sha256.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)),"explicit bounded backend identity required");Ok(())}}
/// Trusted integration port, not a provider declaration or a qualification
/// certificate. Implementations may use only these held files, never reopen paths.
pub trait DescriptorPropertyBackend {
    fn identity(&self)->BackendIdentity;
    fn observe(&self,file:&File)->Result<MacApfsCrossCopyPropertiesV1>;
    /// Failure may leave changed destination metadata. Never remove a source or
    /// retry this call implicitly. The host always performs independent readback.
    fn install(&self,source:&File,destination:&File,properties:&MacApfsCrossCopyPropertiesV1)->Result<()>;
}
/// Honest default. This does not supply empty/absent ACL evidence on failure.
pub struct UnavailableBackend;
impl DescriptorPropertyBackend for UnavailableBackend {
    fn identity(&self)->BackendIdentity{BackendIdentity {schema:"optiflow.descriptor-property-backend.v1".into(),implementation_id:"unavailable".into(),source_sha256:"0".repeat(64)}}
    fn observe(&self,_:&File)->Result<MacApfsCrossCopyPropertiesV1>{bail!("reviewed safe native descriptor ACL backend unavailable")}
    fn install(&self,_:&File,_:&File,_:&MacApfsCrossCopyPropertiesV1)->Result<()>{bail!("native descriptor ACL backend unavailable; no metadata operation performed")}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {pub device_id:u64,pub inode:u64,pub links:u64,pub byte_count:u64,pub mode:u32,pub uid:u32,pub gid:u32,pub modified:Timestamp,pub changed:Timestamp,pub content_blake3:String}
/// Fields are private and cannot be deserialized into an observation token.
#[derive(Debug,Clone)]
pub struct Observation {backend:BackendIdentity,subject:Subject,properties:ReviewedProperties}
impl Observation {
    pub fn subject(&self)->&Subject{&self.subject}
    pub fn properties(&self)->&ReviewedProperties{&self.properties}
    pub fn backend(&self)->&BackendIdentity{&self.backend}
    pub fn evidence(&self)->Result<Evidence>{let mut e=Evidence {schema:"optiflow.descriptor-property-observation.v1".into(),backend:self.backend.clone(),subject:self.subject.clone(),properties:self.properties.snapshot().clone(),property_fingerprint:self.properties.fingerprint().into(),evidence_fingerprint:String::new(),native_qualified:false,source_removal_authority:false};e.evidence_fingerprint=fingerprint(&e)?;Ok(e)}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {pub schema:String,pub backend:BackendIdentity,pub subject:Subject,pub properties:MacApfsCrossCopyPropertiesV1,pub property_fingerprint:String,pub evidence_fingerprint:String,pub native_qualified:bool,pub source_removal_authority:bool}
/// Record validation does not construct a live Observation or establish freshness.
impl Evidence {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=4_194_304,"descriptor observation document exceeds4MiB");let value:serde_json::Value=serde_json::from_slice(bytes)?;ensure!(value.is_object()&&value["backend"].is_object()&&value["subject"].is_object()&&value["subject"]["modified"].is_object()&&value["subject"]["changed"].is_object(),"observation records must be objects");property_evidence::parse(&serde_json::to_vec(&value["properties"])?)?;let e:Self=serde_json::from_slice(bytes)?;e.validate()?;Ok(e)}
    pub fn validate(&self)->Result<()>{self.backend.validate()?;let p=property_evidence::review(self.properties.clone())?;let mut copy=self.clone();copy.evidence_fingerprint.clear();ensure!(self.schema=="optiflow.descriptor-property-observation.v1"&&self.property_fingerprint==p.fingerprint()&&self.evidence_fingerprint==fingerprint(&copy)?&&!self.native_qualified&&!self.source_removal_authority&&self.subject.inode>0&&self.subject.changed.nanoseconds<1_000_000_000&&self.subject.links==1&&self.subject.byte_count>0&&self.subject.byte_count<=268_435_456&&self.subject.content_blake3.len()==64&&self.subject.content_blake3.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)),"descriptor record identity or scope differs");bind_scalars(&self.subject,p.snapshot())}
}
fn fingerprint<T:Serialize>(v:&T)->Result<String>{let value=serde_json::to_value(v)?;Ok(blake3::hash(&serde_json::to_vec(&value)?).to_hex().to_string())}
fn check(cancel:&impl Fn()->bool)->Result<()>{ensure!(!cancel(),"descriptor property operation cancelled");Ok(())}
#[cfg(unix)]
fn signature(file:&File)->Result<Subject>{use std::os::unix::fs::MetadataExt;let m=file.metadata()?;ensure!(m.is_file()&&m.ino()>0&&m.nlink()==1&&m.len()>0,"descriptor requires a singly linked nonempty regular file");ensure!((0..1_000_000_000).contains(&m.mtime_nsec())&&(0..1_000_000_000).contains(&m.ctime_nsec()),"native timestamp is not normalized");Ok(Subject {device_id:m.dev(),inode:m.ino(),links:m.nlink(),byte_count:m.len(),mode:m.mode(),uid:m.uid(),gid:m.gid(),modified:Timestamp {seconds:m.mtime(),nanoseconds:m.mtime_nsec() as u32},changed:Timestamp {seconds:m.ctime(),nanoseconds:m.ctime_nsec() as u32},content_blake3:String::new()})}
#[cfg(not(unix))] fn signature(_:&File)->Result<Subject>{bail!("descriptor properties require Unix held-file observations")}
#[cfg(unix)]
fn content(file:&File,maximum:u64,cancel:&impl Fn()->bool)->Result<Subject>{use std::os::unix::fs::FileExt;ensure!((1..=268_435_456).contains(&maximum),"invalid descriptor byte bound");check(cancel)?;let before=signature(file)?;ensure!(before.byte_count<=maximum,"descriptor content exceeds selected bound");let mut offset=0u64;let mut hash=blake3::Hasher::new();let mut buffer=[0u8;65_536];loop{check(cancel)?;let n=file.read_at(&mut buffer,offset).context("bounded positional descriptor read failed")?;if n==0{break;}offset=offset.checked_add(n as u64).context("descriptor byte overflow")?;ensure!(offset<=maximum&&offset<=before.byte_count,"descriptor grew during observation");hash.update(&buffer[..n]);}ensure!(offset==before.byte_count&&signature(file)?==before,"descriptor content or metadata changed during read");let mut result=before;result.content_blake3=hash.finalize().to_hex().to_string();check(cancel)?;Ok(result)}
#[cfg(not(unix))]fn content(_:&File,_:u64,_:&impl Fn()->bool)->Result<Subject>{bail!("descriptor content observation unsupported")}
fn bind_scalars(subject:&Subject,p:&MacApfsCrossCopyPropertiesV1)->Result<()>{ensure!(subject.mode==p.mode&&subject.uid==p.uid&&subject.gid==p.gid&&subject.modified==p.mtime,"backend properties disagree with actual descriptor ownership/mode/mtime");Ok(())}
fn native_extra(file:&File,p:&MacApfsCrossCopyPropertiesV1)->Result<()> {
    #[cfg(target_os="macos")] {use std::os::macos::fs::MetadataExt;let m=file.metadata()?;ensure!(m.st_birthtime()==p.birthtime.seconds&&m.st_birthtime_nsec() as u32==p.birthtime.nanoseconds&&m.st_flags()==p.bsd_flags,"backend birthtime/flags disagree with Darwin descriptor");}
    #[cfg(not(target_os="macos"))]let _=(file,p);
    Ok(())
}
/// Two exact content/property observations around the same held descriptor.
/// Backend identity is pinned; no self-declared record is accepted as a token.
pub fn observe(file:&File,backend:&impl DescriptorPropertyBackend,maximum:u64,cancel:&impl Fn()->bool)->Result<Observation>{let identity=backend.identity();identity.validate()?;let first=content(file,maximum,cancel)?;let properties=property_evidence::review(backend.observe(file)?)?;bind_scalars(&first,properties.snapshot())?;native_extra(file,properties.snapshot())?;check(cancel)?;let second=property_evidence::review(backend.observe(file)?)?;ensure!(second==properties&&content(file,maximum,cancel)?==first&&backend.identity()==identity,"descriptor/backend/ACL/property window changed");Ok(Observation {backend:identity,subject:first,properties})}
pub fn recheck(file:&File,backend:&impl DescriptorPropertyBackend,expected:&Observation,maximum:u64,cancel:&impl Fn()->bool)->Result<()>{let now=observe(file,backend,maximum,cancel)?;ensure!(now.backend==expected.backend&&now.subject==expected.subject&&now.properties==expected.properties,"current descriptor observation differs from bound subject");Ok(())}
/// Install only after exact copied content exists on an explicitly supplied
/// distinct destination. This API never publishes/removes either namespace.
pub fn preserve_on_copy(source:&File,destination:&File,backend:&impl DescriptorPropertyBackend,expected:&Observation,maximum:u64,cancel:&impl Fn()->bool)->Result<Observation>{recheck(source,backend,expected,maximum,cancel)?;let target=content(destination,maximum,cancel)?;ensure!((target.device_id,target.inode)!=(expected.subject.device_id,expected.subject.inode)&&target.content_blake3==expected.subject.content_blake3&&target.byte_count==expected.subject.byte_count,"property copy needs a distinct exact-content destination");check(cancel)?;
    backend.install(source,destination,expected.properties.snapshot())?;
    // A failed install/readback is never retried or converted into success.
    recheck(source,backend,expected,maximum,cancel)?;let copied=observe(destination,backend,maximum,cancel)?;ensure!(copied.properties==expected.properties&&copied.subject.content_blake3==expected.subject.content_blake3&&copied.subject.byte_count==expected.subject.byte_count&&backend.identity()==expected.backend,"destination property/content readback differs after installation");Ok(copied)
}
#[cfg(all(test,target_os="linux"))] mod tests;
