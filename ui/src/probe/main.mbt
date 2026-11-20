// A probe of how words sit in a capsule on this host (not shipped: built only to measure). Boxes 36 high, 60 apart from
// 40 down, each words centred in it as a button's are (a row lining them up at their middles): a line as tall as @sf's
// button sets (20) and as tall as the font (0), in Chinese and Latin; @sf's own button and pill; and large single
// glyphs. The baseline mbui sets in a 20 line said on the log, with the font's ascent and descent behind it.

///|
fn probe_box(col : @view.View, words : String, size : Double, line : Double) -> Unit {
  col.box(fn(b) {
    b.row(Is(true)).lines_up(Is(Some(Middle))).width(Is(Some(300.0))).height(Is(Some(36.0))).ground(Is(0x303030FFL)) |> ignore
    b.box(fn(s) { s.grow(Is(1.0)) |> ignore }) |> ignore
    b.box(fn(w) {
      w
      .words(Is(words), ink=0xFFFFFFFFL, look={ ..@view.words_plain, size, weight: 500, line, wrap: false })
      |> ignore
    })
    |> ignore
    b.box(fn(s) { s.grow(Is(1.0)) |> ignore }) |> ignore
  })
  |> ignore
}

///|
fn main {
  @hostapi.window_setup(800.0, 700.0, 800.0, 700.0)
  @ui.run_tree("probe", fn(root) {
    @sf.set_dark(Some(true))
    @ui.set_tree_theme(@sf.ui_theme)
    root.grow(Is(1.0)).ground(Is(0x000000FFL)).padding(Is((40.0, 40.0, 40.0, 40.0))).gap(Is(24.0)) |> ignore
    probe_box(root, "配置 检查更新", 13.0, 20.0)
    probe_box(root, "配置 检查更新", 13.0, 0.0)
    probe_box(root, "Check Update", 13.0, 20.0)
    probe_box(root, "Check Update", 13.0, 0.0)
    root.box(fn(r) {
      r.height(Is(Some(36.0))).row(Is(true)).lines_up(Is(Some(Middle))) |> ignore
      @sf.button_node(r, fn() { "配置 检查更新" }, fn() {  }) |> ignore
    })
    |> ignore
    root.box(fn(r) {
      r.height(Is(Some(36.0))).row(Is(true)).lines_up(Is(Some(Middle))) |> ignore
      @sf.pill_node(r, fn() { "纯逻辑" }) |> ignore
    })
    |> ignore
    probe_box(root, "国", 26.0, 0.0)
    probe_box(root, "H", 26.0, 0.0)
    println(
      "PROBE baseline in a 20 line at 13/500: \{@ui.line_baseline(20.0, size=13.0, weight=500)}; in a 36 line at 26/500: \{@ui.line_baseline(36.0, size=26.0, weight=500)}; web_lines \{@ui.web_lines_on()}",
    )
  })
}
