use tauri::http::{Method, Request, Response, StatusCode, header};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder, Wry};

pub(crate) fn serve(
    context: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    if request.method() != Method::GET {
        responder.respond(empty(StatusCode::METHOD_NOT_ALLOWED));
        return;
    }
    let (file, content_type) = match request.uri().path() {
        "/fonts.css" => ("fonts.css", "text/css; charset=utf-8"),
        "/google-sans-code.ttf" => ("google-sans-code.ttf", "font/ttf"),
        "/wqy-zenhei.ttf" => ("wqy-zenhei.ttf", "font/ttf"),
        _ => {
            responder.respond(empty(StatusCode::NOT_FOUND));
            return;
        }
    };

    let directory = context.app_handle().path().resource_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = directory
            .and_then(|path| std::fs::read(path.join("fonts").join(file)).map_err(Into::into));
        match bytes {
            Ok(bytes) => responder.respond(
                Response::builder()
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .body(bytes)
                    .expect("fixed bundled font headers"),
            ),
            Err(error) => {
                log::error!("Could not load bundled font resource {file}: {error}");
                responder.respond(empty(StatusCode::INTERNAL_SERVER_ERROR));
            }
        }
    });
}

fn empty(status: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = status;
    response
}
