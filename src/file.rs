// use std::fs::File;
// use std::io::{copy, Error, ErrorKind, Result};
// use std::net::{TcpListener, TcpStream};
// use std::path::Path;
// use std::thread;
//
// use crate::transport::tcp::TcpHandler;
//
// trait HasHostname {
//     fn hostname(&self) -> String;
// }
//
// #[derive(Clone, Debug)]
// pub struct FileHandler {}
//
// impl FileHandler {
//     pub fn new() -> Self {
//         Self {}
//     }
//
//     pub fn upload<P: AsRef<Path>>(
//         &self,
//         hostname: String,
//         path: P,
//     ) -> Result<TcpHandler> {
//         let path = path.as_ref().to_owned();
//         if !path.is_file() {
//             return Err(Error::new(ErrorKind::NotFound, "file not found"));
//         }
//
//         let listener = TcpListener::bind("0.0.0.0:0")?;
//         let port = listener.local_addr()?.port();
//         let interrupt = listener.try_clone()?;
//
//         thread::spawn(move || {
//             while let Ok((mut stream, _)) = listener.accept() {
//                 let mut file = File::open(&path).unwrap();
//                 thread::spawn(move || {
//                     if let Err(err) = copy(&mut file, &mut stream) {
//                         // error!("while uploading file: {}", err);
//                     }
//                 });
//             }
//         });
//
//         Ok(TcpHandler::new(
//             format!("tcp://{}:{}", hostname, port),
//             interrupt,
//         ))
//     }
//
//     pub fn download<P: AsRef<Path>>(&self, url: &str, path: P) -> Result<()> {
//         // NOTE: tcp://host:port
//         if !url.starts_with("tcp://") {
//             return Err(Error::new(
//                 ErrorKind::InvalidInput,
//                 "url doesn't start with tcp://'",
//             ));
//         }
//
//         let addr = &url[6..];
//         let mut stream = TcpStream::connect(addr)?;
//         let mut file = File::create(&path)?;
//         // debug!("downloading file from tcp://{} to {:?}", addr, path.as_ref());
//
//         copy(&mut stream, &mut file)?;
//
//         Ok(())
//     }
// }
