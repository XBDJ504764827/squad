use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const AUTH_PACKET_TYPE: i32 = 3;
const COMMAND_PACKET_TYPE: i32 = 2;
const AUTH_RESPONSE_PACKET_TYPE: i32 = 2;
const COMMAND_RESPONSE_PACKET_TYPE: i32 = 0;
const AUTH_REQUEST_ID: i32 = 1;
const COMMAND_REQUEST_ID: i32 = 2;

pub async fn validate_rcon_credentials(ip: &str, port: u16, password: &str) -> Result<(), String> {
    connect_authenticated(ip, port, password).await.map(|_| ())
}

pub async fn execute_rcon_command(
    ip: &str,
    port: u16,
    password: &str,
    command: &str,
) -> Result<String, String> {
    let mut stream = connect_authenticated(ip, port, password).await?;

    write_packet(&mut stream, COMMAND_REQUEST_ID, COMMAND_PACKET_TYPE, command)
        .await
        .map_err(map_command_io_error)?;

    for _ in 0..4 {
        let (request_id, packet_type, body) = read_packet(&mut stream).await.map_err(map_command_io_error)?;
        if request_id == COMMAND_REQUEST_ID && packet_type == COMMAND_RESPONSE_PACKET_TYPE {
            return Ok(body);
        }
    }

    Err("未收到有效的 RCON 命令响应".to_string())
}

async fn write_packet(
    stream: &mut TcpStream,
    request_id: i32,
    packet_type: i32,
    body: &str,
) -> io::Result<()> {
    let body_bytes = body.as_bytes();
    let packet_size = 4 + 4 + body_bytes.len() + 2;
    let packet_size = i32::try_from(packet_size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too large"))?;

    stream.write_i32_le(packet_size).await?;
    stream.write_i32_le(request_id).await?;
    stream.write_i32_le(packet_type).await?;
    stream.write_all(body_bytes).await?;
    stream.write_all(&[0, 0]).await?;
    stream.flush().await?;

    Ok(())
}

async fn read_packet(stream: &mut TcpStream) -> io::Result<(i32, i32, String)> {
    let packet_size = stream.read_i32_le().await?;
    if packet_size < 10 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid rcon packet size",
        ));
    }

    let request_id = stream.read_i32_le().await?;
    let packet_type = stream.read_i32_le().await?;
    let body_size = usize::try_from(packet_size - 10)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid rcon packet body size"))?;
    let mut buffer = vec![0_u8; body_size];
    stream.read_exact(&mut buffer).await?;
    let mut terminator = [0_u8; 2];
    stream.read_exact(&mut terminator).await?;

    Ok((request_id, packet_type, String::from_utf8_lossy(&buffer).to_string()))
}

async fn connect_authenticated(ip: &str, port: u16, password: &str) -> Result<TcpStream, String> {
    let address = format!("{ip}:{port}");

    let mut stream = timeout(Duration::from_secs(5), TcpStream::connect(&address))
        .await
        .map_err(|_| "连接服务器超时，请检查 IP 和 RCON 端口".to_string())?
        .map_err(|_| "无法连接到目标服务器，请检查 IP 和 RCON 端口".to_string())?;

    write_packet(&mut stream, AUTH_REQUEST_ID, AUTH_PACKET_TYPE, password)
        .await
        .map_err(map_auth_io_error)?;

    for _ in 0..2 {
        let (request_id, packet_type, _) = read_packet(&mut stream).await.map_err(map_auth_io_error)?;

        if packet_type == AUTH_RESPONSE_PACKET_TYPE {
            if request_id == -1 {
                return Err("RCON 密码验证失败，请确认密码是否正确".to_string());
            }

            if request_id == AUTH_REQUEST_ID {
                return Ok(stream);
            }
        }
    }

    Err("未收到有效的 RCON 验证响应".to_string())
}

fn map_auth_io_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::UnexpectedEof => "RCON 连接被服务器关闭".to_string(),
        io::ErrorKind::TimedOut => "RCON 响应超时，请稍后重试".to_string(),
        _ => format!("RCON 验证失败：{error}"),
    }
}

fn map_command_io_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::UnexpectedEof => "RCON 命令执行时连接被服务器关闭".to_string(),
        io::ErrorKind::TimedOut => "RCON 命令响应超时，请稍后重试".to_string(),
        _ => format!("RCON 命令执行失败：{error}"),
    }
}
