#[path = "../src/persistent_snapshot.rs"]
mod persistent_snapshot;
#[path = "../src/ruby_vm.rs"]
mod ruby_vm;
#[path = "../src/sl_message.rs"]
mod sl_message;

pub fn json_response<T: serde::Serialize>(data: T) -> tide::Response {
    let mut response = tide::Response::new(tide::StatusCode::Ok);
    response.set_body(serde_json::to_string(&data).unwrap());
    response.insert_header("Content-Type", "application/json; charset=utf-8");
    response
}

fn main() -> tide::Result<()> {
    ruby_vm::start().map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error))?;
    async_std::task::block_on(async {
        let mut app = tide::new();
        sl_message::mount(&mut app)?;
        let address = std::env::var("SL_MESSAGE_PREVIEW_ADDR").unwrap_or_else(|_| "127.0.0.1:8781".into());
        println!("SL Message preview: http://{address}/slmessage");
        app.listen(address).await?;
        Ok(())
    })
}