//! Built-in sample documents and templates (original text, generated in code).

use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::{Align, Border, BorderStyle, Borders, CharProps, NumRef, Rgb, TabAlign, TabLeader, TabStop, TextColor};
use wordcraft_doc::{Block, Document, ListKind, Paragraph, PartKind, Table, para_block};

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

fn styled(text: &str, style: &str) -> Paragraph {
    para(text).styled(style)
}

/// Append text with formatting to a paragraph.
fn add(p: &mut Paragraph, text: &str, props: CharProps) {
    let at = p.len();
    let _ = p.insert_text(at, text, &props);
}

fn bold() -> CharProps {
    CharProps { bold: Some(true), ..Default::default() }
}
fn italic() -> CharProps {
    CharProps { italic: Some(true), ..Default::default() }
}

/// The showcase document: a short studio handbook with headings, lists, a table, a quote,
/// a footer with page numbers.
pub fn sample_document() -> Document {
    let mut d = Document::new();
    d.core.title = "The Open Studio Handbook".into();
    d.core.creator = "WordCraft".into();
    let mut b: Vec<Block> = Vec::new();
    b.push(Block::Para(styled("The Open Studio Handbook", "Title")));
    b.push(Block::Para(styled("How a small community of makers shares space, tools and time", "Subtitle")));
    let mut intro = para("Every Thursday evening the old print shop on Alder Street fills with people who make things. ");
    add(&mut intro, "Painters", bold());
    add(&mut intro, " set up by the north windows, ", CharProps::default());
    add(&mut intro, "letterpress", italic());
    add(
        &mut intro,
        " regulars claim the back room, and somebody always brings bread. This handbook collects what we have learned about running a shared studio: the routines that keep it pleasant, the tools we pool, and the small agreements that let thirty strangers become a crew.",
        CharProps::default(),
    );
    b.push(Block::Para(intro));
    b.push(Block::Para(styled("Why share a studio?", "Heading1")));
    b.push(Block::Para(para("Rent is the obvious reason, but it is not the best one. A shared room puts your work next to work you would never have seen otherwise. A ceramicist explains glaze chemistry to a comic artist; a photographer borrows a weaver's loom for a still life. The cross-talk is the point.")));
    let mut quote = styled("A studio is a place where unfinished things are welcome.", "Quote");
    quote.props.space_before = Some(12.0);
    b.push(Block::Para(quote));
    b.push(Block::Para(styled("What we agree on", "Heading2")));
    let mut num = d.numbering.clone();
    let bullets = num.add_list(ListKind::Bullet);
    let steps = num.add_list(ListKind::Numbered);
    d.numbering = num;
    for (lead, rest) in [
        ("Clean as you go. ", "Leave a table the way you would like to find it."),
        ("Label what is yours. ", "Unlabelled supplies on the commons shelf are fair game."),
        ("Ask before you borrow. ", "Even from the shelf, a quick note helps everyone keep track."),
        ("Quiet hours ", "run from 9 to 11 on weekday mornings."),
    ] {
        let mut p = Paragraph::new().styled("ListParagraph");
        add(&mut p, lead, bold());
        add(&mut p, rest, CharProps::default());
        p.props.numbering = Some(NumRef { num: bullets, level: 0 });
        b.push(Block::Para(p));
    }
    b.push(Block::Para(styled("Shared equipment", "Heading2")));
    b.push(Block::Para(para("The table below lists what the membership owns together and who keeps each item in working order.")));
    let rows = [
        ["Equipment", "Location", "Steward", "Booking"],
        ["Etching press", "Back room", "Mara O.", "Sign-up sheet"],
        ["Kiln (cone 10)", "Yard shed", "Theo P.", "48 h notice"],
        ["Large-format printer", "Office", "Ines R.", "Calendar"],
        ["Light table", "North windows", "Anyone", "First come"],
    ];
    let mut t = Table::new(rows.len(), 4, 468.0);
    t.props.style = Some("GridTable4AccentBlue".into());
    for (r, row) in rows.iter().enumerate() {
        for (c, txt) in row.iter().enumerate() {
            if let Some(cell) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                cell.blocks = vec![para_block(para(txt))];
            }
        }
    }
    if let Some(r) = t.rows.first_mut() {
        r.props.header = true;
    }
    b.push(Block::Table(t));
    b.push(Block::Para(styled("Opening the studio", "Heading2")));
    for s in [
        "Switch on the main breaker by the door.",
        "Open both skylights unless it is raining.",
        "Start the kettle. This step is not optional.",
        "Check the booking sheet for the kiln and press.",
    ] {
        let mut p = para(s).styled("ListParagraph");
        p.props.numbering = Some(NumRef { num: steps, level: 0 });
        b.push(Block::Para(p));
    }
    b.push(Block::Para(styled("Membership", "Heading1")));
    let mut m = para("Membership is ");
    add(&mut m, "pay what you can", CharProps { bold: Some(true), color: Some(TextColor::Rgb(Rgb(0x15, 0x60, 0x82))), ..Default::default() });
    add(
        &mut m,
        ", reviewed every season. New members shadow a regular for their first two visits, and everyone takes one cleaning shift a month. If you have never made anything before, you are exactly who we hoped would show up.",
        CharProps::default(),
    );
    b.push(Block::Para(m));
    let mut h = para("Questions go to the front desk, or to the notice board by the sink. ");
    add(
        &mut h,
        "Visit the community board",
        CharProps { style: Some("Hyperlink".into()), link: Some("https://getartcraft.com/".into()), ..Default::default() },
    );
    add(&mut h, " for events.", CharProps::default());
    b.push(Block::Para(h));
    d.body = b.into_iter().map(std::sync::Arc::new).collect();
    // Footer: centred page number.
    let mut f = Paragraph::new().styled("Footer");
    f.props.align = Some(Align::Center);
    let _ = f.insert_object(0, InlineObject::Field { instr: "PAGE".into(), result: "1".into(), locked: false }, &CharProps::default());
    let fid = d.add_part(PartKind::Footer, vec![para_block(f)]);
    d.last_section.footers.default = Some(fid);
    let mut hd = styled("The Open Studio Handbook\t\tSpring edition", "Header");
    hd.props.borders = Some(Borders {
        bottom: Some(Border { style: BorderStyle::Single, width: 0.5, color: Some(Rgb(0xBF, 0xBF, 0xBF)), space: 4.0 }),
        ..Default::default()
    });
    let hid = d.add_part(PartKind::Header, vec![para_block(hd)]);
    d.last_section.headers.default = Some(hid);
    d
}

/// A block-style letter (US English).
pub fn letter() -> Document {
    letter_in("en-US")
}

/// A letter for the region of `locale`: German (DIN 5008 order, A4) for `de-*`, US English
/// otherwise; paper, margins and language from [`crate::region::localize_new`].
pub fn letter_in(locale: &str) -> Document {
    let mut d = Document::new();
    crate::region::localize_new(&mut d, locale);
    let date = crate::cmd::insert::format_date(crate::dates::long_date_picture(locale), locale);
    let lines: Vec<Paragraph> = if crate::region::language(locale) == "de" {
        let mut dated = para(&format!("Musterstadt, {date}"));
        dated.props.align = Some(Align::Right);
        let mut subject = Paragraph::new();
        add(&mut subject, "Worum es in diesem Schreiben geht", bold());
        vec![
            styled("Ihr Name", "Title"),
            para("Musterstraße 1 · 12345 Musterstadt · ihr.name@example.com"),
            para(""),
            para("Name der Empfängerin oder des Empfängers"),
            para("Firma"),
            para("Straße und Hausnummer"),
            para("PLZ Ort"),
            para(""),
            dated,
            para(""),
            subject,
            para(""),
            para("Sehr geehrte Damen und Herren,"),
            para(
                "beginnen Sie mit einem Satz, der den Anlass Ihres Schreibens nennt. Halten Sie die Absätze kurz und konkret, und schließen Sie mit dem, was als Nächstes geschehen soll.",
            ),
            para("Vielen Dank für Ihre Zeit."),
            para("Mit freundlichen Grüßen"),
            para(""),
            para("Ihr Name"),
        ]
    } else {
        vec![
            styled("Your Name", "Title"),
            para("123 Your Street · City, ST 00000 · you@example.com"),
            para(""),
            para(&date),
            para(""),
            para("Recipient Name"),
            para("Title, Company"),
            para("Street Address"),
            para(""),
            para("Dear Recipient,"),
            para(
                "Start with a sentence that says why you are writing. Keep paragraphs short and specific, and close with what you would like to happen next.",
            ),
            para("Thank you for your time."),
            para("Sincerely,"),
            para(""),
            para("Your Name"),
        ]
    };
    d.body = lines.into_iter().map(para_block).collect();
    d
}

/// A one-page résumé (US English).
pub fn resume() -> Document {
    resume_in("en-US")
}

/// A one-page résumé for the region of `locale` (German for `de-*`).
pub fn resume_in(locale: &str) -> Document {
    let mut d = Document::new();
    crate::region::localize_new(&mut d, locale);
    let german = crate::region::language(locale) == "de";
    let (name, line) = if german {
        ("Alex Becker", "Illustratorin · alex@example.com · portfolio.example.com")
    } else {
        ("Alex Rivera", "Illustrator · alex@example.com · portfolio.example.com")
    };
    let sections = if german {
        vec![
            ("Berufserfahrung", vec![("Leitende Illustratorin, Hafenverlag", "seit 2021"), ("Freiberufliche Illustratorin", "2016 – 2021")]),
            ("Ausbildung", vec![("Bachelor of Arts Illustration, Kunsthochschule an der Küste", "2016")]),
            ("Kenntnisse", vec![("Tusche, Gouache, digitale Malerei, Buchgestaltung, Schriftgestaltung", "")]),
        ]
    } else {
        vec![
            ("Experience", vec![("Lead Illustrator, Harbor Books", "2021 – present"), ("Freelance Illustrator", "2016 – 2021")]),
            ("Education", vec![("BFA Illustration, Coastal College of Art", "2016")]),
            ("Skills", vec![("Ink, gouache, digital painting, book layout, lettering", "")]),
        ]
    };
    let right = crate::region::text_width(&d);
    let mut b: Vec<Block> = vec![Block::Para(styled(name, "Title")), Block::Para(para(line))];
    for (head, items) in sections {
        b.push(Block::Para(styled(head, "Heading1")));
        for (what, when) in items {
            let mut p = para(what);
            p.props.tabs = Some(vec![TabStop { pos: right, align: TabAlign::Right, leader: TabLeader::None }]);
            if !when.is_empty() {
                add(&mut p, &format!("\t{when}"), italic());
            }
            b.push(Block::Para(p));
        }
    }
    d.body = b.into_iter().map(std::sync::Arc::new).collect();
    d
}

/// A report with a cover heading and sections (US English).
pub fn report() -> Document {
    report_in("en-US")
}

/// A report for the region of `locale` (German for `de-*`).
pub fn report_in(locale: &str) -> Document {
    let mut d = Document::new();
    crate::region::localize_new(&mut d, locale);
    let t: [&str; 11] = if crate::region::language(locale) == "de" {
        [
            "Titel des Berichts",
            "Untertitel oder Datum",
            "Zusammenfassung",
            "Fassen Sie die Ergebnisse in zwei oder drei Sätzen zusammen.",
            "Hintergrund",
            "Beschreiben Sie den Zusammenhang und die Frage, die der Bericht beantwortet.",
            "Ergebnisse",
            "Erstes Ergebnis",
            "Erläutern Sie die Belege.",
            "Empfehlungen",
            "Führen Sie auf, was als Nächstes geschehen soll.",
        ]
    } else {
        [
            "Report Title",
            "Subtitle or date",
            "Summary",
            "Summarise the findings in two or three sentences.",
            "Background",
            "Describe the context and the question the report answers.",
            "Findings",
            "First finding",
            "Explain the evidence.",
            "Recommendations",
            "List what should happen next.",
        ]
    };
    let [title, subtitle, summary, summary_text, background, background_text, findings, first, first_text, recs, recs_text] = t;
    let b: Vec<Block> = vec![
        Block::Para(styled(title, "Title")),
        Block::Para(styled(subtitle, "Subtitle")),
        Block::Para(styled(summary, "Heading1")),
        Block::Para(para(summary_text)),
        Block::Para(styled(background, "Heading1")),
        Block::Para(para(background_text)),
        Block::Para(styled(findings, "Heading1")),
        Block::Para(styled(first, "Heading2")),
        Block::Para(para(first_text)),
        Block::Para(styled(recs, "Heading1")),
        Block::Para(para(recs_text)),
    ];
    d.body = b.into_iter().map(std::sync::Arc::new).collect();
    d
}

/// A blank document for the region of `locale` (A4 and German for `de-DE`).
pub fn blank_in(locale: &str) -> Document {
    let mut d = Document::new();
    crate::region::localize_new(&mut d, locale);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_valid() {
        for d in [sample_document(), letter(), resume(), report(), letter_in("de-DE"), resume_in("de-DE"), report_in("de-DE")] {
            assert!(d.word_count() > 5);
            let l = wordcraft_layout::layout(&d, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
            assert!(!l.pages.is_empty());
        }
    }

    #[test]
    fn german_templates_are_german_and_a4() {
        for d in [blank_in("de-DE"), letter_in("de-DE"), resume_in("de-DE"), report_in("de-DE")] {
            let s = d.sections().first().map(|(_, s)| (*s).clone()).unwrap_or_default();
            assert_eq!(wordcraft_geom::paper_name(s.page_w, s.page_h), Some("A4"));
            assert_eq!(d.styles.default_chr.lang.as_deref(), Some("de-DE"));
        }
        assert!(letter_in("de-DE").plain_text(wordcraft_doc::StoryRef::Body).contains("Mit freundlichen Grüßen"));
        // The résumé's right tab sits at the right margin of the A4 page.
        let r = resume_in("de-DE");
        let tab = r.body.iter().find_map(|b| match &**b {
            Block::Para(p) => p.props.tabs.as_ref().and_then(|t| t.first()).map(|t| t.pos),
            _ => None,
        });
        assert!(tab.is_some_and(|t| (t - crate::region::text_width(&r)).abs() < 0.01));
        assert_eq!(letter(), letter_in("en-US"));
    }
}
