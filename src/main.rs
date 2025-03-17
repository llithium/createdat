use anyhow::Result;
use createdat::run;
use inquire::ui::{RenderConfig, Styled};

#[tokio::main]
async fn main() -> Result<()> {
    inquire::set_global_render_config(get_render_config());
    run().await?;
    Ok(())
}

fn get_render_config() -> RenderConfig<'static> {
    RenderConfig::<'_> {
        unselected_checkbox: Styled::new("○"),
        selected_checkbox: Styled::new("●"),
        ..Default::default()
    }
}
