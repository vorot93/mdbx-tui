use clap::Parser;
use cursive::{
    align::HAlign,
    theme::{BorderStyle, Color, Palette, PaletteColor},
    traits::Scrollable,
    views::{Dialog, ListView, SelectView, TextView},
    Cursive,
};
use expanded_pathbuf::ExpandedPathBuf;
use libmdbx::NoWriteMap;
use once_cell::sync::Lazy;
use std::{borrow::Cow, path::Path};

#[derive(Parser)]
#[clap(name = "mdbx-tui", about = "TUI for MDBX storage engine")]
pub struct Opt {
    #[clap(long)]
    env_path: ExpandedPathBuf,
}

fn read_max_dbs(path: &Path) -> anyhow::Result<Vec<String>> {
    let mut b = libmdbx::Database::<NoWriteMap>::new();
    b.set_flags(::libmdbx::DatabaseFlags {
        mode: ::libmdbx::Mode::ReadOnly,
        ..Default::default()
    });
    let env = b.open(path)?;
    let tx = env.begin_ro_txn()?;
    let main_db = tx.open_table(None)?;
    let mut cursor = tx.cursor(&main_db)?;
    let mut total_tables = Vec::new();
    while let Some((table, _)) = cursor.next_nodup::<Vec<u8>, ()>()? {
        total_tables.push(String::from_utf8(table)?);
    }
    Ok(total_tables)
}

static ENVIRONMENT: Lazy<(libmdbx::Database<NoWriteMap>, Vec<String>)> = Lazy::new(|| {
    let opt = Opt::parse();

    let tables = read_max_dbs(&opt.env_path).unwrap();
    let mut b = libmdbx::Database::<NoWriteMap>::new();
    b.set_max_tables(tables.len());
    b.set_flags(::libmdbx::DatabaseFlags {
        mode: ::libmdbx::Mode::ReadOnly,
        ..Default::default()
    });
    let b = b.open(&opt.env_path).unwrap();
    (b, tables)
});

fn main() -> anyhow::Result<()> {
    let tx = ENVIRONMENT.0.begin_ro_txn()?;

    // Creates the cursive root - required for every application.
    let mut siv = cursive::default();

    let mut theme = siv.current_theme().clone();
    theme.shadow = !theme.shadow;
    theme.borders = BorderStyle::Simple;
    let mut palette = Palette::default();
    palette[PaletteColor::Background] = Color::TerminalDefault;
    theme.palette = palette;

    siv.set_theme(theme);

    // Creates a dialog with a single "Quit" button
    siv.add_layer({
        let mut select = SelectView::<String>::new()
            // Center the text horizontally
            .h_align(HAlign::Center)
            // Use keyboard to jump to the pressed letters
            .autojump();
        select.add_all_str(ENVIRONMENT.1.clone());
        select.set_on_submit(move |siv: &mut Cursive, table: &String| {
            siv.pop_layer();
            let db = tx.open_table(Some(table)).unwrap();
            siv.add_layer({
                let mut view = ListView::new();

                for res in tx
                    .cursor(&db)
                    .unwrap()
                    .iter_start::<Cow<[u8]>, Cow<[u8]>>()
                    .take(20)
                {
                    let (k, v) = res.unwrap();
                    view.add_child(&hex::encode(k), TextView::new(hex::encode(v)))
                }

                view
                // Dialog::around(TextView::new(format!("Selected {}", table)))
                //     .title("mdbx-tui")
                //     .button("Quit", |s| s.quit()),
            })
        });
        Dialog::around(select.scrollable())
    });

    // Starts the event loop.
    siv.run();

    Ok(())
}
