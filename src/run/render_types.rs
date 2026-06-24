use super::*;

pub(crate) struct RenderCtx {
    pub(crate) theme: crate::ui::theme::ThemeSpec,
    pub(crate) main: Rect,
    pub(crate) footer: Rect,
    pub(crate) header_rows: u16,
    pub(crate) scrollbar_in_main: bool,
}

pub(crate) struct TableLayout {
    pub(crate) list_frame_area: Rect,
    pub(crate) preview_frame_area: Option<Rect>,
    pub(crate) table_area: Rect,
    pub(crate) can_draw_scrollbar: bool,
    pub(crate) list_area: Rect,
}


