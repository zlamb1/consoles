use crate::simple::Result;

#[derive(Clone, Copy, Debug)]
pub struct CellGrid {
    /// Number of columns.
    pub width: usize,
    /// Number of rows.
    pub height: usize,
    /// Current column position in grid.
    pub x: usize,
    /// Current row position in grid.
    pub y: usize,
    /// Whether to consume a newline or not.
    pub deferred_wrap: bool,
}

impl CellGrid {
    pub fn cell_count(&self) -> usize {
        self.width * self.height
    }

    pub fn cell_index(&self) -> usize {
        self.y * self.width + self.x
    }

    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            x: 0,
            y: 0,
            deferred_wrap: false,
        }
    }
}

#[derive(Debug)]
pub struct CellWriter<
    'a,
    Context,
    I: Copy + Default,
    GetIndex: FnMut(&Context, &CellGrid) -> I,
    WriteCell: FnMut(&mut Context, &CellGrid, I, u8) -> Result<()>,
    Scroll: FnMut(&mut Context, &CellGrid) -> Result<()>,
> {
    pub ctx: &'a mut Context,
    pub index: I,
    pub get_index: GetIndex,
    pub write_cell: WriteCell,
    pub scroll: Scroll,
}

impl<
    'a,
    Context,
    I: Copy + Default,
    GetIndex: FnMut(&Context, &CellGrid) -> I,
    WriteCell: FnMut(&mut Context, &CellGrid, I, u8) -> Result<()>,
    Scroll: FnMut(&mut Context, &CellGrid) -> Result<()>,
> CellWriter<'a, Context, I, GetIndex, WriteCell, Scroll>
{
    pub fn new(
        ctx: &'a mut Context,
        get_index: GetIndex,
        write_cell: WriteCell,
        scroll: Scroll,
    ) -> Self {
        Self {
            ctx,
            index: Default::default(),
            get_index,
            write_cell,
            scroll,
        }
    }

    pub fn write(&'a mut self, cell_grid: &mut CellGrid, s: &[u8]) -> Result<usize> {
        let len = s.len();
        let width = cell_grid.width;
        let height = cell_grid.height;

        if width == 0 || height == 0 {
            return Ok(len);
        }

        let cell_count = cell_grid.cell_count();
        let reset_index = (height - 1) * width;

        let mut cell_index = cell_grid.cell_index();
        let mut i = 0usize;

        self.index = (self.get_index)(self.ctx, cell_grid);

        'outer: while i < len {
            let target = i + core::cmp::min(len - i, cell_count - cell_index);
            while i < target {
                let ch = s[i];
                i += 1;

                let deferred_wrap = cell_grid.deferred_wrap;

                if deferred_wrap {
                    cell_grid.deferred_wrap = false;
                }

                match ch {
                    0x8 => {
                        if cell_index == 0 {
                            continue;
                        }
                        if cell_grid.x > 0 {
                            cell_grid.x -= 1;
                        } else {
                            cell_grid.x = width - 1;
                            cell_grid.y -= 1;
                        }
                        cell_index -= 1;
                        self.index = (self.get_index)(self.ctx, cell_grid);
                        continue 'outer;
                    }
                    b'\n' => {
                        if deferred_wrap && cell_index == cell_count - 1 {
                            continue;
                        }
                        cell_grid.x = 0;
                        cell_grid.y += 1;
                        if cell_grid.y == height {
                            cell_grid.y = height - 1;
                            (self.scroll)(self.ctx, cell_grid)?;
                        }
                        cell_index = cell_grid.y * width;
                        self.index = (self.get_index)(self.ctx, cell_grid);
                        continue 'outer;
                    }
                    b'\r' => {
                        cell_grid.x = 0;
                        cell_index = cell_grid.y * width;
                        self.index = (self.get_index)(self.ctx, cell_grid);
                        continue 'outer;
                    }
                    _ => {}
                }

                if deferred_wrap && cell_index == cell_count - 1 {
                    // Printable character. Needs scroll.
                    cell_grid.x = 0;
                    cell_index = reset_index;
                    self.index = (self.get_index)(self.ctx, cell_grid);
                    (self.scroll)(self.ctx, cell_grid)?;
                }

                (self.write_cell)(self.ctx, cell_grid, self.index, ch)?;

                cell_grid.x += 1;
                if cell_grid.x == width {
                    cell_grid.x = 0;
                    cell_grid.y += 1;
                    cell_grid.deferred_wrap = true;
                }
                cell_index += 1;

                if cell_grid.y < height {
                    self.index = (self.get_index)(self.ctx, cell_grid);
                }
            }
            if cell_index == cell_count {
                // Cursor logically floats at last cell.
                cell_grid.x = width - 1;
                cell_grid.y = height - 1;
                cell_index -= 1;
                self.index = (self.get_index)(self.ctx, cell_grid);
            }
        }

        Ok(len)
    }
}
