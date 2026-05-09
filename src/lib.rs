pub mod cell_grid;
pub mod color;
pub mod cursor;
pub mod fb;
pub mod font;

/// A module for "simple" consoles. Simple consoles are basic consoles
/// that consume no memory resources and only support write-through.
/// They are useful for constrained or early boot environments.
pub mod simple;
