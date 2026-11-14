use paper_desktop::{MainWindow, config::Config, ui};
use slint::ComponentHandle;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_args(std::env::args_os().skip(1), |key| std::env::var_os(key))
        .map_err(std::io::Error::other)?;
    let window = MainWindow::new()?;
    let _controller = ui::attach(&window, config);
    window.run()?;
    Ok(())
}
