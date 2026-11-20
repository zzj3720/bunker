// Bunker's looks: the confirmed dark, industrial design (charcoal grounds, warm sand-gold for the main action, green
// and amber for states), written over mbui's headless controls (@headless.button, toggle, popover, text fields) as
// mbui's three layers have it: behaviour theirs, all that is seen here.

///|
pub let c_app : Int64 = 0x111315FFL

///|
pub let c_sidebar : Int64 = 0x16191CFFL

///|
pub let c_panel : Int64 = 0x1B1F23FFL

///|
pub let c_elevated : Int64 = 0x22272CFFL

///|
pub let c_hover : Int64 = 0x282E35FFL

///|
pub let c_border : Int64 = 0x2C323AFFL

///|
pub let c_border_strong : Int64 = 0x3C444FFFL

///|
pub let c_text : Int64 = 0xECEEF0FFL

///|
pub let c_text2 : Int64 = 0x9AA4B0FFL

///|
pub let c_muted : Int64 = 0x6C7682FFL

///|
pub let c_accent : Int64 = 0xC8A258FFL

///|
pub let c_accent_hover : Int64 = 0xD8B46AFFL

///|
pub let c_accent_soft : Int64 = 0xC8A25824L

///|
pub let c_green : Int64 = 0x4FB87AFFL

///|
pub let c_green_soft : Int64 = 0x4FB87A24L

///|
pub let c_amber : Int64 = 0xE09F3EFFL

///|
pub let c_amber_soft : Int64 = 0xE09F3E24L

///|
pub let c_red : Int64 = 0xE05D5DFFL

///|
pub let c_red_soft : Int64 = 0xE05D5D24L

///|
pub let c_blue : Int64 = 0x5B9BD5FFL

///|
pub let c_blue_soft : Int64 = 0x5B9BD524L

///|
let c_titlebar : Int64 = 0x131619FFL

///|
let c_actionbar : Int64 = 0x14171AFFL

///|
let c_console : Int64 = 0x0D0F11FFL

///|
let c_console_text : Int64 = 0xC8D1DCFFL

///|
let c_ink_on_accent : Int64 = 0x111315FFL

///|
let c_tag_muted : Int64 = 0x262B31FFL

///|
fn wl(size : Double, weight? : Int = 400, wrap? : Bool = false) -> @view.WordsLook {
  { ..@view.words_plain, size, weight, wrap }
}

///|
/// Words in `parent`: `size`, `weight`, `ink`.
fn text(
  parent : @view.View,
  s : String,
  size? : Double = 13.0,
  weight? : Int = 400,
  ink? : Int64 = c_text,
  wrap? : Bool = false,
) -> @view.View {
  parent.box(fn(b) { b.words(Is(s), ink~, look=wl(size, weight~, wrap~)) |> ignore })
}

///|
/// Words as they are now.
fn text_by(
  parent : @view.View,
  s : () -> String,
  size? : Double = 13.0,
  weight? : Int = 400,
  ink? : Int64 = c_text,
  ink_by? : () -> Int64,
  wrap? : Bool = false,
) -> @view.View {
  parent.box(fn(b) {
    b.words(By(s), ink~, look=wl(size, weight~, wrap~)) |> ignore
    match ink_by {
      Some(f) => b.ink(By(f)) |> ignore
      None => ()
    }
  })
}

///|
fn spacer(parent : @view.View) -> Unit {
  parent.box(fn(b) { b.grow(Is(1.0)) |> ignore }) |> ignore
}

///|
fn row(parent : @view.View, gap : Double, make : (@view.View) -> Unit) -> @view.View {
  parent.box(fn(r) {
    r.row(Is(true)).lines_up(Is(Some(Middle))).gap(Is(gap)) |> ignore
    make(r)
  })
}

///|
/// A panel: the design's card.
fn card(parent : @view.View, make : (@view.View) -> Unit) -> @view.View {
  parent.box(fn(c) {
    c
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .padding(Is((16.0, 14.0, 16.0, 14.0)))
    |> ignore
    make(c)
  })
}

///|
#derive(Eq)
pub(all) enum Tone {
  Primary
  Plain
  Ghost
  Danger
}

///|
/// A button of the design's: `Primary` sand-gold, `Plain` raised, `Ghost` bare, `Danger` red words.
fn button(
  parent : @view.View,
  label : String,
  press : () -> Unit,
  tone? : Tone = Plain,
  small? : Bool = false,
  disabled? : () -> Bool = fn() { false },
  label_by? : () -> String,
  grows? : Bool = false,
) -> @view.View {
  let words = match label_by {
    Some(f) => f
    None => fn() { label }
  }
  @headless.button(parent, words, fn(_) { press() }, disabled~, fn(v, b) {
    let (g, lit, ink, edge) = match tone {
      Primary => (c_accent, c_accent_hover, c_ink_on_accent, c_accent)
      Plain => (c_elevated, c_hover, c_text, c_border_strong)
      Ghost => (0L, c_hover, c_text2, 0L)
      Danger => (c_elevated, c_red_soft, c_red, c_border_strong)
    }
    let h = if small { 28.0 } else { 36.0 }
    let px = if small { 10.0 } else if tone == Primary { 18.0 } else { 14.0 }
    v
    .row(Is(true))
    .lines_up(Is(Some(Middle)))
    .height(Is(Some(h)))
    .padding(Is((px, 0.0, px, 0.0)))
    .radius(Is(6.0))
    .ground(By(fn() { if !b.disabled() && (b.hovered() || b.pressed()) { lit } else { g } }))
    .edge(Is((if edge == 0L { 0.0 } else { 1.0 }, edge)))
    .alpha(By(fn() { if b.disabled() { 0.45 } else { 1.0 } }))
    .cursor(Is(Some(Hand)))
    |> ignore
    if grows {
      v.grow(Is(1.0)) |> ignore
    }
    v.box(fn(t) { t.words(By(words), ink~, look=wl(if small { 12.0 } else { 13.0 }, weight=600)) |> ignore }) |> ignore
  })
}

///|
/// A switch of the design's: green while on.
fn switch(
  parent : @view.View,
  label : String,
  on : () -> Bool,
  turn : (Bool) -> Unit,
  label_by? : () -> String,
) -> @view.View {
  let words = match label_by {
    Some(f) => f
    None => fn() { label }
  }
  @headless.toggle(parent, Switch, words, on, turn, fn(v, t) {
    v
    .width(Is(Some(40.0)))
    .height(Is(Some(22.0)))
    .radius(Is(11.0))
    .ground(By(fn() { if t.on() { c_green } else { 0x3A4048FFL } }))
    .cursor(Is(Some(Hand)))
    .padding(Is((2.0, 2.0, 2.0, 2.0)))
    .row(Is(true))
    |> ignore
    v.box(fn(k) { k.grow(By(fn() { if t.on() { 1.0 } else { 0.0 } })) |> ignore }) |> ignore
    v.box(fn(k) {
      k
      .width(Is(Some(18.0)))
      .height(Is(Some(18.0)))
      .radius(Is(9.0))
      .ground(Is(0xFFFFFFFFL))
      .shadow(Is((4.0, 1.0, 0x00000066L)))
      |> ignore
    })
    |> ignore
  })
}

///|
#derive(Eq)
pub(all) enum TagTone {
  Green
  Amber
  Blue
  Gold
  Red
  Muted
}

///|
fn tag_colors(t : TagTone) -> (Int64, Int64) {
  match t {
    Green => (c_green_soft, c_green)
    Amber => (c_amber_soft, c_amber)
    Blue => (c_blue_soft, c_blue)
    Gold => (c_accent_soft, c_accent)
    Red => (c_red_soft, c_red)
    Muted => (c_tag_muted, c_text2)
  }
}

///|
/// A tag: a state or a side, in its colour on its soft ground.
fn tag(parent : @view.View, s : String, t : TagTone) -> @view.View {
  let (g, ink) = tag_colors(t)
  parent.box(fn(b) {
    b.ground(Is(g)).radius(Is(5.0)).padding(Is((7.0, 2.0, 7.0, 2.0))) |> ignore
    b.words(Is(s), ink~, look=wl(11.0, weight=600)) |> ignore
  })
}

///|
/// A tag whose words and tone follow what they say.
fn tag_by(parent : @view.View, s : () -> String, t : () -> TagTone) -> @view.View {
  // (empty: nothing drawn and no room taken)
  parent.box(fn(b) {
    b
    .ground(By(fn() { if s() == "" { 0L } else { tag_colors(t()).0 } }))
    .radius(Is(5.0))
    .padding(By(fn() { if s() == "" { (0.0, 0.0, 0.0, 0.0) } else { (7.0, 2.0, 7.0, 2.0) } }))
    |> ignore
    b.words(By(s), look=wl(11.0, weight=600)).ink(By(fn() { tag_colors(t()).1 })) |> ignore
  })
}

///|
/// A pill: a count or a word beside a nav item.
fn pill_by(parent : @view.View, s : () -> String, active : () -> Bool) -> @view.View {
  parent.box(fn(b) {
    b
    .ground(By(fn() { if active() { 0xC8A25838L } else { c_elevated } }))
    .radius(Is(99.0))
    .padding(Is((7.0, 1.0, 7.0, 1.0)))
    |> ignore
    b.words(By(s), look=wl(11.0)).ink(By(fn() { if active() { c_accent } else { c_text2 } })) |> ignore
  })
}

///|
/// A dot: green running, amber on its way, grey stopped.
fn dot(parent : @view.View, color : () -> Int64) -> @view.View {
  parent.box(fn(d) {
    d
    .width(Is(Some(8.0)))
    .height(Is(Some(8.0)))
    .radius(Is(4.0))
    .ground(By(color))
    .shadow(By(fn() { if color() == c_green { (8.0, 0.0, 0x4FB87AB3L) } else { (0.0, 0.0, 0L) } }))
    |> ignore
  })
}

///|
/// A one-line field of words in its frame: `placeholder`; `edited` told as people type; `submitted` on Enter.
fn line_field(
  parent : @view.View,
  field : @ui.Field,
  placeholder : String,
  width? : Double,
  edited? : (String) -> Unit = fn(_) {  },
  submitted? : (String) -> Unit = fn(_) {  },
  label? : String = "",
) -> @view.View {
  parent.box(fn(b) {
    b
    .row(Is(true))
    .lines_up(Is(Some(Middle)))
    .height(Is(Some(34.0)))
    .padding(Is((12.0, 0.0, 12.0, 0.0)))
    .radius(Is(6.0))
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border_strong)))
    .cursor(Is(Some(IBeam)))
    |> ignore
    match width {
      Some(w) => b.width(Is(Some(w))) |> ignore
      None => b.grow(Is(1.0)).basis(Is(Some(0.0))).min_width(Is(Some(0.0))) |> ignore
    }
    @ui.field(
      b,
      field,
      placeholder~,
      font_size=12.5,
      max_lines=1,
      color=c_text,
      placeholder_color=c_muted,
      caret_color=c_accent,
      enter_sends=true,
      edited~,
      submitted~,
      label=if label == "" { placeholder } else { label },
    )
    .grow(Is(1.0))
    |> ignore
  })
}

///|
/// What a press on a row of a list does: its ground lit while pointed at.
fn hover_ground(v : @view.View, rest : Int64, lit : Int64) -> Unit {
  v.ground(By(fn() { if v.pointed() { lit } else { rest } })) |> ignore
}
