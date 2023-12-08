use std::io::{Cursor, Read};

use crate::{Error, Server};

/// download the contents of a file on the server.
pub async fn fetch_file(server: &Server, path: &str) -> Result<Vec<u8>, Error> {
    let mut ftp = server.ftp.write().await;
    Ok(ftp.simple_retr(path)?.into_inner())
}

/// upload the contents of a file on the server.
pub async fn upload_file(server: &Server, path: &str, contents: &[u8]) -> Result<(), Error> {
    let mut ftp = server.ftp.write().await;
    ftp.put(path, &mut Cursor::new(contents))?;
    Ok(())
}
