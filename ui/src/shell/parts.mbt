// Bunker's looks: the dark tactical design (charcoal grounds, warm sand-gold, green and amber for states). Its controls
// are still.fail's component library's (`@sf`, AFK-surf/monoid's libs/sf-ui, over mbui's headless components): its
// buttons, switches, fields, pills, segmented choices, state dots, sidebar items, panels and toasts, in its dark theme
// with a sand-gold colour theme of its own (shell.mbt sets them). What is here is the layout's: words, rows, cards.

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
/// A button: @sf's (a pill 36 high in still.fail's look), in its tone.
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
  ignore(small)
  let words = match label_by {
    Some(f) => f
    None => fn() { label }
  }
  let b = @sf.button_node(parent, words, press, tone=match tone {
    Primary => Primary
    Plain => Plain
    Ghost => Ghost
    Danger => Danger
  }, disabled~)
  if grows {
    b.grow(Is(1.0)) |> ignore
  }
  b
}

///|
/// A switch: @sf's.
fn switch(
  parent : @view.View,
  label : String,
  on : () -> Bool,
  turn : (Bool) -> Unit,
  label_by? : () -> String,
) -> Unit {
  let words = match label_by {
    Some(f) => f
    None => fn() { label }
  }
  @sf.switch_node(parent, on, turn, words)
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
fn pill_tone(t : TagTone) -> @sf.PillTone {
  match t {
    Green => Green
    Amber => Amber
    Blue => Blue
    Gold => Accented
    Red => Red
    Muted => Neutral
  }
}

///|
/// Words in a capsule: the frame (`b`: its ground, edge, corner and padding) lays its words out in a box of their own,
/// so the padding is round them, and centres them across it.
fn capsule_words(b : @view.View, words : @view.Live[String], look : @view.WordsLook, ink : () -> Int64) -> Unit {
  b.row(Is(true)).lines_up(Is(Some(Middle))) |> ignore
  b.box(fn(w) { w.words(words, look~).ink(By(ink)) |> ignore }) |> ignore
}

///|
/// A tag: a state or a side, @sf's pill in its tone.
fn tag(parent : @view.View, s : String, t : TagTone) -> Unit {
  @sf.pill_node(parent, fn() { s }, tone=pill_tone(t)) |> ignore
}

///|
/// A tag whose words and tone follow what they say (none while its words are none).
fn tag_by(parent : @view.View, s : () -> String, t : () -> TagTone) -> Unit {
  parent.show(along_row=true, fn() { s() != "" }, fn(x) {
    x.pick(t, fn(slot, tone) { @sf.pill_node(slot, s, tone=pill_tone(tone)) |> ignore })
  })
}

///|
/// A count or a word beside a sidebar item: @sf's pill, in the accent while its item is chosen.
fn pill_by(parent : @view.View, s : () -> String, active : () -> Bool) -> Unit {
  parent.pick(active, fn(slot, on) { @sf.pill_node(slot, s, tone=if on { Accented } else { Neutral }) |> ignore })
}

///|
/// What `make` puts in `v` centred both ways: `v` a row lining its own up at their middles, room shared either side.
fn centred(v : @view.View, make : (@view.View) -> Unit) -> Unit {
  v.row(Is(true)).lines_up(Is(Some(Middle))) |> ignore
  spacer(v)
  make(v)
  spacer(v)
}

///|
/// A server's state as @sf's dot: online, busy (starting, stopping), error, or waiting.
fn dot(parent : @view.View, state : () -> String) -> Unit {
  @sf.state_dot_node(parent, fn() {
    match state() {
      "running" | "external" => "online"
      "starting" | "stopping" => "busy"
      "failed" => "error"
      _ => "waiting"
    }
  })
}

///|
/// A one-line field of words: @sf's input, `width` wide (none: as wide as its row leaves it); `edited` told as people
/// type, `submitted` on Enter.
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
    match width {
      Some(w) => b.width(Is(Some(w))).shrink(Is(0.0)) |> ignore
      None => b.grow(Is(1.0)).basis(Is(Some(0.0))).min_width(Is(Some(0.0))) |> ignore
    }
    @sf.input_node(b, field, placeholder~, label=if label == "" { placeholder } else { label }, edited~, submitted~) |> ignore
  })
}

///|
/// What a press on a row of a list does: its ground lit while pointed at.
fn hover_ground(v : @view.View, rest : Int64, lit : Int64) -> Unit {
  v.ground(By(fn() { if v.pointed() { lit } else { rest } })) |> ignore
}
