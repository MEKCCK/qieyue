//! One module per screen. Each `show` is a pure function over [`crate::app::App`]:
//! it draws the view and posts commands, never owns state of its own.

pub(crate) mod book;
pub(crate) mod category;
pub(crate) mod chrome;
pub(crate) mod empty;
pub(crate) mod home;
pub(crate) mod library;
pub(crate) mod manga;
pub(crate) mod nav;
pub(crate) mod rank;
pub(crate) mod reader;
pub(crate) mod results;
pub(crate) mod series;
pub(crate) mod widgets;
