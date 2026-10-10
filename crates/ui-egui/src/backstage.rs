//! File tab (Backstage): Home/New, Open, Info, Save As, Print, Export, Options, About.

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::theme::{APP_COLOR, Tokens, medium, regular, semibold};
use crate::{WordApp, icons};

const PAGES: [(&str, &str); 9] = [
    ("home", "Home"),
    ("new", "New"),
    ("open", "Open"),
    ("info", "Info"),
    ("save", "Save"),
    ("saveAs", "Save As"),
    ("print", "Print"),
    ("export", "Export"),
    ("options", "Options"),
];

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("backstage_nav")
        .exact_size(200.0)
        .frame(egui::Frame::NONE.fill(APP_COLOR).inner_margin(egui::Margin { left: 0, right: 0, top: 12, bottom: 12 }))
        .show(ui, |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(200.0, 40.0), Sense::click());
            icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(r.min.x + 26.0, r.center().y), vec2(18.0, 18.0)),
                "chevronLeft",
                egui::Color32::WHITE,
                egui::Color32::WHITE,
            );
            if resp.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.ui.backstage = false;
                app.canvas.want_focus = true;
            }
            ui.add_space(6.0);
            for (id, label) in PAGES {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 38.0), Sense::click());
                let active = app.ui.backstage_page == id;
                if active {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                }
                ui.painter().text(
                    pos2(r.min.x + 22.0, r.center().y),
                    Align2::LEFT_CENTER,
                    tl!(label),
                    if active { semibold(14.0) } else { medium(14.0) },
                    egui::Color32::WHITE,
                );
                if resp.clicked() {
                    match id {
                        "save" => {
                            let _ = app.run("file.save", json!({}));
                        }
                        "saveAs" => app.save_as_dialog(),
                        "open" => {
                            app.ui.backstage_page = id.into();
                        }
                        _ => app.ui.backstage_page = id.into(),
                    }
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                for (label, id) in [("About", "about"), ("Discord community", "discord")] {
                    let (r, resp) = ui.allocate_exact_size(vec2(200.0, 32.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                    }
                    ui.painter().text(pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, tl!(label), regular(13.0), egui::Color32::WHITE);
                    if resp.clicked() {
                        if id == "discord" {
                            let _ = app.run("ui.discord", json!({}));
                        } else {
                            app.dialog = crate::dialogs::Dialog::open("about", app);
                        }
                    }
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(egui::Margin::symmetric(40, 30))).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| match app.ui.backstage_page.as_str() {
            "new" | "home" => new_page(app, ui),
            "open" => open_page(app, ui),
            "info" => info_page(app, ui),
            "print" | "export" => export_page(app, ui),
            "options" => options_page(app, ui),
            _ => new_page(app, ui),
        });
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(tl!(s)).font(semibold(26.0)));
    ui.add_space(16.0);
}

fn template_tile(ui: &mut Ui, app: &mut WordApp, label: &str, template: &str) {
    let t = Tokens::get(ui.ctx());
    ui.vertical(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(150.0, 194.0), Sense::click());
        ui.painter().rect(
            r,
            2.0,
            egui::Color32::WHITE,
            Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border_strong }),
            egui::StrokeKind::Inside,
        );
        let ppp = ui.ctx().pixels_per_point();
        let key = format!("tpl:{template}:{ppp}");
        let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(&key))).or_else(|| {
            let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
            s.locale = app.session.locale.clone();
            let _ = s.run("file.new", &json!({"template": template}));
            let l = s.export_layout();
            let page = l.pages.first()?;
            let img = wordcraft_render::render_page(&s.doc, page, 150.0 / page.w * ppp, &Default::default());
            let px = img.to_straight();
            let h = ui.ctx().load_texture(
                &key,
                egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &px),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(&key), h.clone()));
            Some(h)
        });
        if let Some(h) = tex {
            ui.painter().image(h.id(), r.shrink(1.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        ui.label(egui::RichText::new(tl!(label)).font(regular(12.5)));
        if resp.clicked() {
            let _ = app.run("file.new", json!({"template": template}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    });
}

fn new_page(app: &mut WordApp, ui: &mut Ui) {
    let hour = wordcraft_engine::dates::now_local().hour;
    heading(
        ui,
        if hour < 12 {
            "Good morning"
        } else if hour < 18 {
            "Good afternoon"
        } else {
            "Good evening"
        },
    );
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(18.0, 0.0);
        template_tile(ui, app, "Blank document", "blank");
        template_tile(ui, app, "Studio handbook (sample)", "sample");
        template_tile(ui, app, "Letter", "letter");
        template_tile(ui, app, "Résumé", "resume");
        template_tile(ui, app, "Report", "report");
    });
    ui.add_space(28.0);
    open_list(app, ui);
}

fn open_list(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Recent")).font(semibold(16.0)));
    ui.add_space(6.0);
    if app.ui.recent.is_empty() {
        ui.label(egui::RichText::new(tl!("Documents you open will show up here.")).color(t.text_dim));
    }
    for p in app.ui.recent.clone() {
        let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(p.clone());
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 40.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.hover);
        }
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(22.0, 22.0)), "printLayout", t.icon, t.accent);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 13.0), Align2::LEFT_CENTER, &name, medium(13.0), t.text);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 29.0), Align2::LEFT_CENTER, &p, regular(11.0), t.text_dim);
        if resp.clicked() {
            let _ = app.run("file.open", json!({"path": p}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    }
}

fn open_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Open");
    if ui.button(egui::RichText::new(tl!("📂  Browse…")).font(medium(14.0))).clicked() {
        let _ = app.run("ui.openFileDialog", json!({}));
    }
    ui.add_space(18.0);
    open_list(app, ui);
}

fn info_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Info");
    let info = app.session.run("file.info", &json!({})).unwrap_or_default();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new(tl!("Properties")).font(semibold(15.0)));
        let mut props = app.session.doc.core.clone();
        let mut changed = false;
        egui::Grid::new("props").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            for (l, v) in [("Title", &mut props.title), ("Subject", &mut props.subject), ("Author", &mut props.creator), ("Keywords", &mut props.keywords), ("Category", &mut props.category)] {
                ui.label(tl!(l));
                changed |= ui.text_edit_singleline(v).lost_focus();
                ui.end_row();
            }
        });
        if changed {
            let _ = app.run("file.properties", json!({"title": props.title, "subject": props.subject, "author": props.creator, "keywords": props.keywords, "category": props.category}));
        }
        let ui = &mut cols[1];
        ui.label(egui::RichText::new(tl!("Statistics")).font(semibold(15.0)));
        for (l, k) in [("Pages", "pages"), ("Words", "words"), ("Paragraphs", "paragraphs"), ("Sections", "sections"), ("Comments", "comments")] {
            ui.label(format!("{}: {}", tl!(l), info.get(k).map(|v| v.to_string()).unwrap_or_default()));
        }
        let path = info.get("path").and_then(|v| v.as_str()).unwrap_or(tl!("Not saved yet"));
        ui.label(crate::i18n::fmt(tl!("Location: {path}"), &[("path", path)]));
    });
}

fn export_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, if app.ui.backstage_page == "print" { "Print" } else { "Export" });
    ui.label(tl!("Save a copy in another format. Printing goes through a PDF you can print from any viewer."));
    ui.add_space(12.0);
    for (label, ext) in [
        ("PDF document (*.pdf)", "pdf"),
        ("Word document (*.docx)", "docx"),
        ("OpenDocument Text (*.odt)", "odt"),
        ("Rich Text Format (*.rtf)", "rtf"),
        ("Web page (*.html)", "html"),
        ("Markdown (*.md)", "md"),
        ("Plain text (*.txt)", "txt"),
        ("Page image (*.png)", "png"),
    ] {
        if ui.add(egui::Button::new(egui::RichText::new(tl!(label)).font(medium(13.5))).min_size(vec2(320.0, 34.0))).clicked() {
            let name = format!("{}.{ext}", app.title_stem());
            let picked = app.services.pick_save.as_ref().and_then(|f| f(&name));
            if let Some(path) = picked {
                let r = if ext == "png" { app.run("file.exportPng", json!({"path": path})) } else { app.run("file.saveAs", json!({"path": path})) };
                if r.is_ok() {
                    app.status(crate::i18n::fmt(tl!("Exported {path}"), &[("path", &path)]));
                }
            }
        }
        ui.add_space(4.0);
    }
}

fn options_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Options");
    ui.label(egui::RichText::new(tl!("General")).font(semibold(15.0)));
    language_picker(app, ui);
    ui.add_space(8.0);
    units_picker(app, ui);
    ui.horizontal(|ui| {
        ui.label(tl!("User name:"));
        let mut n = app.session.author.clone();
        if ui.text_edit_singleline(&mut n).changed() {
            app.session.author = n;
        }
    });
    let mut dark = app.ui.dark;
    if ui.checkbox(&mut dark, tl!("Dark mode")).changed() {
        let _ = app.run("ui.dark", json!({"value": dark}));
    }
    ui.checkbox(&mut app.autosave, tl!("AutoSave documents that have been saved"));
    ui.checkbox(&mut app.ui.show_discord, tl!("Show the community button in the title bar"));
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Display")).font(semibold(15.0)));
    let mut marks = app.session.view.marks;
    if ui.checkbox(&mut marks, tl!("Show all formatting marks")).changed() {
        let _ = app.run("view.marks", json!({"value": marks}));
    }
    let mut ruler = app.session.view.ruler;
    if ui.checkbox(&mut ruler, tl!("Show rulers")).changed() {
        let _ = app.run("view.ruler", json!({"value": ruler}));
    }
    ui.add_space(10.0);
    proofing_options(app, ui);
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Agents")).font(semibold(15.0)));
    ui.label(tl!("Every command is available to scripts and AI agents: run `wordcraft-cli mcp` for an MCP server, or start the app with `--control <port>` for the JSON control channel."));
}

/// File ▸ Options ▸ Proofing: the dictionaries found, where to add more, and the optional
/// LanguageTool server for grammar and style.
fn proofing_options(app: &mut WordApp, ui: &mut Ui) {
    ui.label(egui::RichText::new(tl!("Proofing")).font(semibold(15.0)));
    // The folder scan is cached for the page; "Search again" refreshes it.
    let id = egui::Id::new("proofing_dictionaries");
    let mut info: Option<serde_json::Value> = ui.data(|d| d.get_temp(id));
    if info.is_none() {
        info = app.session.run("review.dictionaries", &json!({})).ok();
        ui.data_mut(|d| d.insert_temp(id, info.clone()));
    }
    let info = info.unwrap_or_default();
    let names = |key: &str| -> Vec<String> {
        let mut v: Vec<String> = info[key]
            .as_array()
            .map(|a| {
                // English always uses the built-in tools, so English dictionaries aren't listed.
                a.iter()
                    .filter_map(|x| x["name"].as_str())
                    .filter(|n| !n.starts_with("en"))
                    .map(|n| crate::chrome::language_name(n.trim_end_matches("_frami")))
                    .collect()
            })
            .unwrap_or_default();
        v.dedup();
        v
    };
    let none = tl!("none").to_string();
    let spelling = names("spelling");
    let hyphenation = names("hyphenation");
    ui.label(crate::i18n::fmt(
        tl!("Spelling: English (built in), {dictionaries}"),
        &[("dictionaries", &if spelling.is_empty() { none.clone() } else { spelling.join(", ") })],
    ));
    ui.label(crate::i18n::fmt(
        tl!("Hyphenation: English (built in), {patterns}"),
        &[("patterns", &if hyphenation.is_empty() { none } else { hyphenation.join(", ") })],
    ));
    let lang = wordcraft_engine::region::lang_tag(&app.session.locale).unwrap_or_else(|| "en-US".into());
    if !wordcraft_engine::proof::is_english(Some(&lang)) && wordcraft_engine::proof::hunspell::locate(&lang).is_none() {
        ui.label(
            egui::RichText::new(crate::i18n::fmt(
                tl!("There's no {language} dictionary yet: install your system's Hunspell dictionary package, or add the .aff and .dic files to the folder below."),
                &[("language", &crate::chrome::language_name(&lang))],
            ))
            .color(Tokens::get(ui.ctx()).accent),
        );
    }
    ui.horizontal(|ui| {
        let dir = info["userDir"].as_str().unwrap_or("").to_string();
        ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Your dictionaries folder: {folder}"), &[("folder", &dir)])).small().weak());
        if ui.small_button(tl!("Search again")).clicked() {
            let fresh = app.session.run("review.dictionaries", &json!({"rescan": true})).ok();
            ui.data_mut(|d| d.insert_temp(id, fresh));
        }
    });
    ui.add_space(6.0);
    let status_id = egui::Id::new("language_tool_status");
    // The engine's state is the truth (scripts and agents can switch it too).
    if let Some(u) = wordcraft_engine::proof::languagetool::url() {
        app.ui.language_tool = true;
        app.ui.language_tool_url = u;
    } else {
        app.ui.language_tool = false;
    }
    let mut on = app.ui.language_tool;
    let changed_on = ui.checkbox(&mut on, tl!("Check grammar and style with LanguageTool")).changed();
    let mut url = app.ui.language_tool_url.clone();
    let mut changed_url = false;
    ui.horizontal(|ui| {
        ui.label(tl!("Server:"));
        changed_url = ui.add(egui::TextEdit::singleline(&mut url).desired_width(260.0)).lost_focus() && url != app.ui.language_tool_url;
    });
    if changed_on || changed_url {
        app.ui.language_tool = on;
        app.ui.language_tool_url = url.clone();
        let r = app.session.run("review.languageTool", &json!({"enabled": on, "url": url}));
        let status = match r {
            Ok(v) => v["status"].as_str().unwrap_or("").to_string(),
            Err(e) => format!("unreachable: {e}"),
        };
        ui.data_mut(|d| d.insert_temp(status_id, status));
    }
    if app.ui.language_tool {
        let status: String = ui.data(|d| d.get_temp(status_id)).unwrap_or_default();
        let t = Tokens::get(ui.ctx());
        if status == "ok" {
            ui.label(egui::RichText::new(tl!("Connected.")).small());
        } else if let Some(err) = status.strip_prefix("unreachable: ") {
            ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Not reachable: {error}"), &[("error", err)])).small().color(t.accent));
        }
    }
    ui.label(
        egui::RichText::new(tl!("LanguageTool is a separate, free program. Start its server on this computer, e.g. `java -cp languagetool-server.jar org.languagetool.server.HTTPServer --port 8081`; your text stays on this computer."))
            .small()
            .weak(),
    );
}

/// File ▸ Options ▸ Measurement units: the region's (the default) or one picked by hand.
fn units_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::measure::{AUTO, UNITS};
    let name = |u: &str| match u {
        "cm" => tl!("Centimeters"),
        "mm" => tl!("Millimeters"),
        "in" => tl!("Inches"),
        _ => tl!("Points"),
    };
    let region = UNITS.iter().find(|(_, u)| *u == wordcraft_engine::region::unit(&app.session.locale)).map(|(k, _)| *k).unwrap_or("in");
    let auto_label = crate::i18n::fmt(tl!("Automatic ({unit})"), &[("unit", name(region))]);
    let current = if app.ui.units == AUTO { auto_label.clone() } else { name(&app.ui.units).to_string() };
    ui.horizontal(|ui| {
        ui.label(tl!("Show measurements in units of:"));
        egui::ComboBox::from_id_salt("measurement_units").selected_text(current).width(180.0).show_ui(ui, |ui| {
            if ui.selectable_label(app.ui.units == AUTO, auto_label.as_str()).clicked() {
                let _ = app.run("ui.units", json!({"value": AUTO}));
            }
            for (k, _) in UNITS {
                if ui.selectable_label(app.ui.units == k, name(k)).clicked() {
                    let _ = app.run("ui.units", json!({"value": k}));
                }
            }
        });
    });
}

/// File ▸ Options ▸ Interface language: follow the system (the default) or pick one (#8).
fn language_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::i18n::{AUTO, Lang};
    let system = crate::i18n::system_lang();
    let auto_label = crate::i18n::fmt(tl!("Automatic ({language})"), &[("language", system.name())]);
    let current = if app.ui.language == AUTO { auto_label.clone() } else { Lang::from_pref(&app.ui.language).name().to_string() };
    ui.horizontal(|ui| {
        ui.label(tl!("Interface language:"));
        egui::ComboBox::from_id_salt("interface_language").selected_text(current).width(220.0).show_ui(ui, |ui| {
            if ui.selectable_label(app.ui.language == AUTO, auto_label.as_str()).clicked() {
                let _ = app.run("ui.language", json!({"value": AUTO}));
            }
            for lang in Lang::all() {
                if ui.selectable_label(app.ui.language == lang.code(), lang.name()).clicked() {
                    let _ = app.run("ui.language", json!({"value": lang.code()}));
                }
            }
        });
    });
    ui.label(egui::RichText::new(tl!("Automatic follows your system's language. Menus and commands change; your documents don't.")).small().weak());
}
