//! Narrow literal-loopback HTTP/1 client. One request per connection; no DNS,
//! proxy environment, redirects, credentials, retry or remote fallback.
use std::{collections::BTreeMap,io::{Read,Write},net::{SocketAddr,TcpStream},time::{Instant,Duration}};
use anyhow::{Result,ensure,bail,Context};
fn check(deadline:Instant,cancel:&impl Fn()->bool)->Result<Duration>{ensure!(!cancel(),"local request cancelled");let left=deadline.checked_duration_since(Instant::now()).context("local request deadline exhausted")?;ensure!(!left.is_zero(),"local request deadline exhausted");Ok(left.min(Duration::from_millis(250)))}
struct Reader<'a,F:Fn()->bool>{stream:TcpStream,deadline:Instant,cancel:&'a F}
impl<F:Fn()->bool> Reader<'_,F>{
    fn exact(&mut self,out:&mut [u8])->Result<()> {let mut offset=0;while offset<out.len(){let timeout=check(self.deadline,self.cancel)?;self.stream.set_read_timeout(Some(timeout))?;match self.stream.read(&mut out[offset..]){Ok(0)=>bail!("local response ended early"),Ok(n)=>offset+=n,Err(e) if matches!(e.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock|std::io::ErrorKind::Interrupted)=>{},Err(_)=>bail!("local response read failed")}}Ok(())}
    fn line(&mut self,maximum:usize)->Result<Vec<u8>>{let mut line=Vec::new();loop{ensure!(line.len()<maximum,"local response line exceeds bound");let mut byte=[0];self.exact(&mut byte)?;line.push(byte[0]);if line.ends_with(b"\r\n"){line.truncate(line.len()-2);return Ok(line);}}}
}
pub(super) fn request(endpoint:SocketAddr,method:&str,path:&str,body:&[u8],maximum:usize,deadline:Instant,cancel:&impl Fn()->bool)->Result<Vec<u8>>{
    ensure!(endpoint.ip().is_loopback()&&endpoint.port()!=0&&matches!((method,path),("GET","/api/version")|("GET","/api/tags")|("POST","/api/chat"))&&body.len()<=131_072&&(1..=262_144).contains(&maximum),"unsupported bounded loopback request");
    let mut stream=TcpStream::connect_timeout(&endpoint,check(deadline,cancel)?).context("selected local service unavailable")?;
    let header=format!("{method} {path} HTTP/1.1\r\nHost: {endpoint}\r\nContent-Type: application/json\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",body.len());
    let mut request=header.into_bytes();request.extend_from_slice(body);let mut offset=0;
    while offset<request.len(){stream.set_write_timeout(Some(check(deadline,cancel)?))?;match stream.write(&request[offset..]){Ok(0)=>bail!("local request write stopped"),Ok(n)=>offset+=n,Err(e) if matches!(e.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock|std::io::ErrorKind::Interrupted)=>{},Err(_)=>bail!("local request write failed")}}
    let mut reader=Reader {stream,deadline,cancel};let status=reader.line(256)?;let status=std::str::from_utf8(&status)?;ensure!(status.starts_with("HTTP/1.1 200 ")||status.starts_with("HTTP/1.0 200 "),"local service refused request; no redirect/retry/fallback");
    let mut headers=BTreeMap::<String,String>::new();let mut header_bytes=status.len()+2;
    loop{let line=reader.line(16_384)?;header_bytes+=line.len()+2;ensure!(header_bytes<=16_384&&headers.len()<64,"local response headers exceed bound");if line.is_empty(){break;}
        let line=std::str::from_utf8(&line)?;let (name,value)=line.split_once(':').context("invalid local response header")?;
        ensure!(!name.is_empty()&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')&&!value.chars().any(|c|c.is_control()),"invalid local response header");
        ensure!(headers.insert(name.to_ascii_lowercase(),value.trim().to_string()).is_none(),"duplicate local response header");
    }
    ensure!(headers.get("content-type").is_some_and(|s|s.split(';').next()==Some("application/json"))&&headers.get("content-encoding").is_none_or(|s|s=="identity"),"uncompressed JSON local response required");
    let data=if let Some(length)=headers.get("content-length") {
        ensure!(!headers.contains_key("transfer-encoding")&&!length.is_empty()&&length.bytes().all(|c|c.is_ascii_digit()),"ambiguous local response framing");let length:usize=length.parse()?;ensure!(length>0&&length<=maximum,"local response exceeds byte bound");let mut data=vec![0;length];reader.exact(&mut data)?;data
    }else{
        ensure!(headers.get("transfer-encoding").is_some_and(|s|s.eq_ignore_ascii_case("chunked")),"explicit response length or bounded chunked framing required");
        let mut data=Vec::new();loop{let line=reader.line(32)?;ensure!(!line.is_empty()&&line.len()<=8&&line.iter().all(u8::is_ascii_hexdigit),"unsupported chunk framing");let length=usize::from_str_radix(std::str::from_utf8(&line)?,16)?;
            if length==0 {ensure!(reader.line(2)?.is_empty(),"local response trailers are unsupported");break;}
            let end=data.len().checked_add(length).context("local body size overflow")?;ensure!(end<=maximum,"local response exceeds byte bound");let start=data.len();data.resize(end,0);reader.exact(&mut data[start..])?;let mut crlf=[0;2];reader.exact(&mut crlf)?;ensure!(crlf==*b"\r\n","invalid chunk boundary");
        }ensure!(!data.is_empty(),"empty local response");data
    };check(deadline,cancel)?;Ok(data)
}
