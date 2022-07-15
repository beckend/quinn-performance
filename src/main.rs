mod common;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use common::{make_client_endpoint, make_server_endpoint};
use futures_util::stream::StreamExt;
use quinn::{Connection, Endpoint};
use std::{net::SocketAddr, thread, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
  let addr1: SocketAddr = "127.0.0.1:5000".parse()?;
  let server1_cert = get_server(addr1)?;

  let client = make_client_endpoint("127.0.0.1:0".parse().unwrap(), &[&server1_cert]).unwrap();
  let connection = get_client(&client, addr1).await?;
  let connection_c = std::sync::Arc::new(connection.clone());

  tokio::spawn(async move {
    // for _ in 0..4 {
    //   let connection_c = connection_c.clone();
    //   run_client_task(connection_c).await.unwrap();
    //   println!("done");
    // }
    let connection_c = connection_c.clone();
    run_client_task(connection_c).await.unwrap();
    println!("done");
  })
  .await?;

  client.wait_idle().await;
  client.close(0u32.into(), b"done");
  connection.close(0u32.into(), b"done");

  println!("all done");

  Ok(())
}

fn get_server(addr: SocketAddr) -> Result<Vec<u8>> {
  let (mut incoming, server_cert) = make_server_endpoint(addr).unwrap();

  tokio::spawn(async move {
    let quinn::NewConnection {
      connection,
      mut bi_streams,
      ..
    } = incoming.next().await.unwrap().await.unwrap();

    println!(
      "[server] incoming connection: addr={}",
      connection.remote_address()
    );

    while let Some(stream_incoming) = bi_streams.next().await {
      let stream = match stream_incoming {
        Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
          println!("The peer closed the connection");
          return Ok(());
        }
        Err(err) => {
          return Err(anyhow!("accepting stream failed: {}", err));
        }
        Ok(s) => s,
      };

      println!("handling stream...");

      tokio::spawn(async move { run_server_task(stream).await }).await??;
    }

    connection.close(0u32.into(), b"done");

    Ok(())
  });

  Ok(server_cert)
}

async fn get_client(endpoint: &Endpoint, server_addr: SocketAddr) -> Result<Connection> {
  let connect = endpoint.connect(server_addr, "localhost").unwrap();
  let quinn::NewConnection { connection, .. } = connect.await.unwrap();
  println!("[client] connected: addr={}", connection.remote_address());

  Ok(connection)
}

async fn run_client_task(connection: std::sync::Arc<Connection>) -> Result<()> {
  let (mut send, _) = connection
    .open_bi()
    .await
    .map_err(|e| anyhow!("failed to open stream: {}", e))?;

  for x in 0..1000 {
    send
      .write(format!("hello{}", x).as_bytes())
      .await
      .map_err(|e| anyhow!("failed to send data: {}", e))?;
  }

  thread::sleep(Duration::from_millis(3000));

  Ok(())
}

async fn run_server_task((_, mut recv): (quinn::SendStream, quinn::RecvStream)) -> Result<()> {
  #[rustfmt::skip]
  let mut bufs = [
      Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
      Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
      Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
  ];

  while recv.read_chunks(&mut bufs[..]).await?.is_some() {
    for x in &bufs {
      dbg!(&x);
    }
  }

  println!("server task done");

  Ok(())
}
