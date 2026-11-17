use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{WebContext, WebViewBuilder, WebViewBuilderExtWindows};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::env::args().nth(1).ok_or("缺少隔离 WebView2 数据目录")?;
    println!("webview2-version={}", wry::webview_version()?);
    let data = std::path::PathBuf::from(data);
    std::fs::create_dir_all(&data)?;
    let mut context = WebContext::new(Some(data.canonicalize()?));
    let events = EventLoopBuilder::<f64>::with_user_event().build();
    let proxy = events.create_proxy();
    let window = WindowBuilder::new()
        .with_title("CCC 隔离 WebView2 验收")
        .with_inner_size(LogicalSize::new(1000.0, 720.0))
        .with_visible(false)
        .build(&events)?;
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url("http://127.0.0.1:1450/tools/ui-check/index.html")
        .with_additional_browser_args("--remote-debugging-port=9450 --remote-debugging-address=127.0.0.1 --disable-gpu")
        .with_ipc_handler(move |request| {
            if let Ok(scale) = request.body().parse::<f64>() {
                if [0.9, 1.0, 1.1, 1.25].contains(&scale) {
                    let _ = proxy.send_event(scale);
                }
            }
        })
        .build(&window)?;
    println!("isolated-webview2-ready");
    events.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(scale) => {
                match webview.zoom(scale) {
                    Ok(()) => { let _ = webview.evaluate_script(&format!("window.__nativeScale={scale}")); }
                    Err(error) => eprintln!("native-zoom-failed: {error}"),
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *flow = ControlFlow::Exit,
            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                let _ = webview.set_bounds(wry::Rect {
                    position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                    size: wry::dpi::PhysicalSize::new(size.width, size.height).into(),
                });
            }
            _ => {}
        }
    });
}
