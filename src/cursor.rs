use crate::cell_grid::CellGrid;

#[derive(Clone, Copy)]
pub struct Cursor {
    pub enabled: bool,
    pub visible: bool,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            enabled: true,
            visible: false,
        }
    }
}

pub fn with_cursor_hidden<Context, Blink, Op, R>(
    ctx: &mut Context,
    cell_grid: &mut CellGrid,
    cursor: Cursor,
    mut op: Op,
    mut blink: Blink,
) -> R
where
    Op: FnMut(&mut Context, &mut CellGrid) -> R,
    Blink: FnMut(&mut Context, &CellGrid),
{
    let visible = cursor.visible;
    if visible {
        blink(ctx, cell_grid);
    }
    let r = op(ctx, cell_grid);
    if visible {
        blink(ctx, cell_grid);
    }
    return r;
}
