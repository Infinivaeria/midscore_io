#[path = "../src/persistent_snapshot.rs"]
mod persistent_snapshot;
#[path = "../src/palm_sessions.rs"]
mod palm_sessions;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let mut app = tide::new();
    palm_sessions::mount(&mut app)?;
    let page = include_str!("../src/main.rs").split("app.at(\"/palmorganizer\")")
        .nth(1).unwrap().split("r###\"").nth(1).unwrap().split("\"###").next().unwrap();
    app.at("/palmorganizer").get(move |_| async move {
        let mut response = tide::Response::new(tide::StatusCode::Ok);
        response.set_content_type(tide::http::mime::HTML);
        response.set_body(page);
        Ok(response)
    });
    let address = std::env::var("PALMORGANIZER_PREVIEW_ADDR").unwrap_or_else(|_| "127.0.0.1:8767".into());
    println!("Palm Organizer preview: http://{address}/palmorganizer");
    app.listen(address).await?;
    Ok(())
}