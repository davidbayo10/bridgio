use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Padding},
};

pub fn panel_block(border_style: Style) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Plain)
        .padding(Padding::horizontal(1))
        .border_style(border_style)
}
