// Mod 管理: every mod of the instance, server and client parts as one row (its sides, version, Chinese coverage, a
// switch that turns it off without leaving anything behind, its config, its uninstall); search and filters; checking
// for newer releases; and installing one, in a popover of two steps (an archive let go or a link, then where each of
// its entries goes), the second stepping in where the first was.

///|
fn mods_page(v : @view.View, s : @model.Store, anchor : Ref[@view.View?]) -> Unit {
  v.padding(Is((22.0, 18.0, 22.0, 18.0))).gap(Is(16.0)) |> ignore
  takes_archives(v, s)
  let search = @ui.Field::new()
  row(v, 10.0, fn(t) {
    line_field(t, search, "搜索 Mod 名称、作者、DLL 或目录", width=300.0, edited=fn(q) { s.mods_query.set(q) }) |> ignore
    // (the filters: @sf's segmented choice, each with its count)
    let count = fn(f : (@model.ModItem) -> Bool) { s.mods.get().filter(f).length() }
    let keys = ["all", "on", "off", "update"]
    @sf.seg_node(t, fn() {
      [
        "全部 (\{count(fn(_) { true })})",
        "已启用 (\{count(fn(m) { m.enabled })})",
        "已禁用 (\{count(fn(m) { !m.enabled })})",
        "可更新 (\{count(fn(m) { m.update != "" })})",
      ]
    }, fn() { keys.search(s.mods_filter.get()).unwrap_or(0) }, fn(i) { s.mods_filter.set(keys[i]) }, said="Mod 筛选")
    |> ignore
    spacer(t)
    button(t, "检查更新", fn() { s.check_updates() }, label_by=fn() {
      if s.busy.get() == "checking" {
        "正在检查…"
      } else {
        "检查更新"
      }
    }, disabled=fn() { s.busy.get() != "" || s.current.get() == "" })
    |> ignore
    // (@sf's button that opens what goes beside it: the install popover; a press on it while that is open closes it as
    // it goes down outside it, and opens it anew)
    let trig = @sf.button_opens_node(t, fn() { "+ 安装 Mod" }, fn(_) {
      if s.current.untracked() != "" {
        s.install_open.set(true)
      }
    }, tone=Primary)
    anchor.val = Some(trig)
  })
  |> ignore
  // the table
  v.box(fn(tb) {
    tb
    .grow(Is(1.0))
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .clips(Is(true))
    |> ignore
    tb.box(fn(h) {
      table_row(h)
      h.height(Is(Some(48.0))).edge(Is((0.0, 0L))) |> ignore
      let head = fn(label : String, w : Double?, at_end : Bool) {
        h.box(fn(c) {
          match w {
            Some(x) => c.width(Is(Some(x))) |> ignore
            None => c.grow(Is(1.0)).shrink(Is(1.0)) |> ignore
          }
          // (as its rows' cells: the actions' head at its end, as their buttons are)
          c.row(Is(true)).lines_up(Is(Some(Middle))) |> ignore
          if at_end {
            spacer(c)
          }
          c.box(fn(t) { t.words(Is(label), ink=c_text2, look=wl(12.0, weight=600)) |> ignore }) |> ignore
        })
        |> ignore
      }
      head("启用", Some(col_switch), false)
      head("Mod 名称与说明", None, false)
      head("覆盖范围（成对追踪）", Some(col_sides), false)
      head("版本", Some(col_version), false)
      head("中文状态", Some(col_loc), false)
      head("操作", Some(col_actions), true)
    })
    |> ignore
    tb.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
    tb.box(fn(sv) {
      sv.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).scrolls(@view.Scroll::new(), fn(list) {
      list.grow(Is(1.0)) |> ignore
      list.each(fn() { s.mods_shown() }, fn(m) { m.id }, fn(r, m) { mod_row(r, s, m) })
      list.show(fn() { s.mods_loaded.get() && s.mods_shown().is_empty() }, fn(e) {
        e.padding(Is((20.0, 24.0, 20.0, 24.0))) |> ignore
        text_by(e, fn() {
          if s.mods.get().is_empty() {
            "这个实例还没有 Mod：把 .zip / .7z 拖到这里，或点「+ 安装 Mod」"
          } else {
            "没有符合条件的 Mod"
          }
        }, size=12.5, ink=c_muted)
        |> ignore
      })
    }) |> ignore
    })
    |> ignore
  })
  |> ignore
}

///|
/// An archive let go on `v` (Explorer's files, read whole by the host): its install planned, the popover opened at its
/// second step.
fn takes_archives(v : @view.View, s : @model.Store) -> Unit {
  let dropped : @signals.Signal[@ui.Pick?] = @signals.signal_ref(None)
  @ui.files_onto(v, held=fn(_) {  }, take=fn(_, _) { Some(fn(p) { dropped.set(Some(p)) }) }) |> ignore
  // (how many files the latest drop holds once read: -1 while there is none or it is being read)
  v.reacts(fn() {
    match dropped.get() {
      Some(p) =>
        match p.files() {
          Some(f) => f.length()
          None => -1
        }
      None => -1
    }
  }, at_start=false, fn(n) {
    if n > 0 {
      match @signals.untracked(fn() { dropped.get() }) {
        Some(p) =>
          match p.files() {
            Some(files) => s.plan_bytes(files[0].name, files[0].bytes)
            None => ()
          }
        None => ()
      }
    }
  })
}

///|
let col_switch : Double = 64.0

///|
let col_sides : Double = 170.0

///|
let col_version : Double = 110.0

///|
let col_loc : Double = 96.0

///|
let col_actions : Double = 170.0

///|
fn table_row(r : @view.View) -> Unit {
  r.row(Is(true)).lines_up(Is(Some(Middle))).padding(Is((20.0, 0.0, 20.0, 0.0))).gap(Is(12.0)) |> ignore
}

///|
fn loc_tag(m : @model.ModItem) -> (String, TagTone) {
  match m.loc {
    "full" => ("已汉化", Green)
    "partial" => ("部分汉化", Amber)
    "none" => ("未汉化", Red)
    _ => (if m.server { "纯逻辑" } else { "无需" }, Muted)
  }
}

///|
fn mod_desc(m : @model.ModItem) -> String {
  if !m.enabled {
    "已安全禁用（\{m.files} 个文件暂存于 .bunker/disabled/\{m.id}，可一键恢复）"
  } else if m.update != "" {
    "发现新版本 v\{m.update}（来自 GitHub Releases）"
  } else {
    let parts = []
    if m.author != "" {
      parts.push(m.author)
    }
    if m.loc_ch > 0 {
      parts.push("\{@model.count_text(m.loc_ch)} 条中文词条")
    }
    match m.compat {
      "mismatch" => parts.push("声明的 SPT 版本 \{m.spt_range} 与本实例不符")
      _ => ()
    }
    parts.push(if m.managed { "Bunker 安装 · \{m.files} 个文件已记录清单" } else { "\{m.files} 个文件" })
    parts.join(" · ")
  }
}

///|
fn mod_row(r : @view.View, s : @model.Store, m : @signals.Signal[@model.ModItem]) -> Unit {
  table_row(r)
  // (a fixed height: a list's rows are laid out without measuring their words)
  r.height(Is(Some(76.0))) |> ignore
  hover_ground(r, 0L, 0xFFFFFF06L)
  let confirm = @signals.signal(false)
  r.box(fn(c) {
    c.width(Is(Some(col_switch))) |> ignore
    switch(c, "启用", fn() { m.get().enabled }, fn(on) { s.set_mod_enabled(m.untracked().id, on) }, label_by=fn() {
      "启用 \{m.get().name}"
    })
    |> ignore
  })
  |> ignore
  r.box(fn(c) {
    c.grow(Is(1.0)).shrink(Is(1.0)).gap(Is(4.0)) |> ignore
    text_by(c, fn() { m.get().name }, size=14.0, weight=700, ink_by=fn() { if m.get().enabled { c_text } else { c_muted } })
    .cuts(Is(true))
    |> ignore
    text_by(c, fn() { mod_desc(m.get()) }, size=12.0, ink_by=fn() {
      if m.get().update != "" && m.get().enabled {
        c_amber
      } else if m.get().enabled {
        c_text2
      } else {
        c_muted
      }
    })
    .cuts(Is(true))
    |> ignore
  })
  |> ignore
  r.box(fn(c) {
    c.width(Is(Some(col_sides))).row(Is(true)).gap(Is(6.0)).alpha(By(fn() { if m.get().enabled { 1.0 } else { 0.5 } })) |> ignore
    tag_by(c, fn() { if m.get().server { "服务端" } else { "" } }, fn() { Gold }) |> ignore
    tag_by(c, fn() { if m.get().client { "客户端" } else { "" } }, fn() { Blue }) |> ignore
  })
  |> ignore
  r.box(fn(c) {
    c.width(Is(Some(col_version))) |> ignore
    text_by(c, fn() {
      let v = m.get().version
      if v == "" {
        "—"
      } else if v.has_prefix("v") {
        v
      } else {
        "v\{v}"
      }
    }, size=13.0, ink=c_text2)
    |> ignore
  })
  |> ignore
  r.box(fn(c) {
    c.width(Is(Some(col_loc))).row(Is(true)) |> ignore
    tag_by(c, fn() { loc_tag(m.get()).0 }, fn() { loc_tag(m.get()).1 }) |> ignore
  })
  |> ignore
  r.box(fn(c) {
    c.width(Is(Some(col_actions))).row(Is(true)).gap(Is(8.0)).lines_up(Is(Some(Middle))) |> ignore
    spacer(c)
    c.show(fn() { m.get().update != "" }, fn(x) {
      button(x, "更新", fn() { s.update_mod(m.untracked().id) }, small=true, tone=Primary) |> ignore
    })
    button(c, "配置", fn() {
      let roots = m.untracked().roots
      let owners = roots.map(fn(rt) {
        match rt.rev_find("/") {
          Some(i) => rt[i + 1:].to_owned()
          None => rt
        }
      })
      s.page.set(Config)
      match s.configs.untracked().search_by(fn(cf) { owners.contains(cf.owner) }) {
        Some(i) => s.open_config(s.configs.untracked()[i].path)
        None => s.say("这个 Mod 没有可编辑的配置文件（JSON / JSONC / CFG）")
      }
    }, small=true)
    |> ignore
    button(c, "卸载", fn() {
      if confirm.untracked() {
        confirm.set(false)
        s.uninstall(m.untracked().id)
      } else {
        confirm.set(true)
      }
    }, small=true, tone=Danger, label_by=fn() { if confirm.get() { "确认卸载" } else { "卸载" } }, disabled=fn() {
      s.busy.get() == "uninstalling"
    })
    |> ignore
  })
  |> ignore
}

///|
fn install_popover(p : @view.View, s : @model.Store) -> Unit {
  let url = @ui.Field::new()
  row(p, 8.0, fn(h) {
    h.padding(Is((18.0, 14.0, 14.0, 14.0))) |> ignore
    text(h, "安装新 Mod", size=14.0, weight=700).grow(Is(1.0)) |> ignore
    tag_by(h, fn() {
      if s.install_step.get() == 1 {
        "步骤 1 / 2 · 选择来源"
      } else {
        "步骤 2 / 2 · 路径预览"
      }
    }, fn() { Gold })
    |> ignore
  })
  |> ignore
  p.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
  p.box(fn(b) {
    b.padding(Is((18.0, 14.0, 18.0, 16.0))).gap(Is(12.0)) |> ignore
    // where the steps are
    row(b, 8.0, fn(cr) {
      cr
      .padding(Is((12.0, 8.0, 12.0, 8.0)))
      .ground(Is(c_elevated))
      .edge(Is((1.0, c_border)))
      .radius(Is(6.0))
      |> ignore
      text_by(cr, fn() { "1. 拖入压缩包 / 贴链接" }, size=12.0, weight=600, ink_by=fn() {
        if s.install_step.get() == 1 { c_accent } else { c_text2 }
      })
      |> ignore
      text(cr, "›", size=12.0, ink=c_muted) |> ignore
      text_by(cr, fn() { "2. 智能路径映射确认" }, size=12.0, weight=600, ink_by=fn() {
        if s.install_step.get() == 2 { c_accent } else { c_text2 }
      })
      |> ignore
    })
    |> ignore
    b.pick(fn() { s.install_step.get() }, fn(st, step) {
      st.gap(Is(12.0)) |> ignore
      if step == 1 {
        // an archive (let go on the window or picked) or a release's link
        st.box(fn(zone) {
          zone
          .height(Is(Some(120.0)))
          .radius(Is(8.0))
          .ground(Is(c_console))
          .edge(Is((1.0, c_border_strong)))
          .lines_up(Is(Some(Middle)))
          .gap(Is(10.0))
          .padding(Is((0.0, 22.0, 0.0, 0.0)))
          |> ignore
          takes_archives(zone, s)
          text_by(zone, fn() {
            if s.busy.get() == "planning" {
              "正在读取压缩包并分析目录结构…"
            } else {
              "把 .zip / .7z 拖到这里"
            }
          }, size=13.0, weight=600, ink=c_text)
          |> ignore
          button(zone, "选择压缩包", fn() {
            @ui.pick(Files, on_got=fn(files) {
              match files.get(0) {
                Some(f) => s.plan_bytes(f.name, f.bytes)
                None => ()
              }
            })
            |> ignore
          }, small=true, disabled=fn() { s.busy.get() == "planning" })
          |> ignore
        })
        |> ignore
        row(st, 8.0, fn(u) {
          line_field(u, url, "粘贴 GitHub Releases 链接（github.com/作者/仓库）", label="Mod 链接", submitted=fn(t) { s.plan_url(t) }) |> ignore
          button(u, "解析", fn() { s.plan_url(url.text()) }, disabled=fn() { s.busy.get() == "planning" }) |> ignore
        })
        |> ignore
        text(st, "自动去掉多余的顶层目录，SPT/user/mods 与 SPT_Runtime/user/mods 按本实例自动映射", size=11.5, ink=c_muted) |> ignore
      } else {
        // where each entry goes
        let plan = fn() {
          match s.plan.get() {
            Some(x) => x
            None =>
              @model.Plan::{
                id: "",
                archive: "",
                name: "",
                version: "",
                target: "",
                replaces: "",
                adapted: false,
                mappings: [],
                conflicts: [],
                skipped: [],
                warnings: [],
                files: 0,
              }
          }
        }
        row(st, 8.0, fn(t) {
          text_by(t, fn() { plan().archive }, size=14.0, weight=700).grow(Is(1.0)).cuts(Is(true)) |> ignore
          tag_by(t, fn() {
            if plan().adapted {
              "已自动适配 \{plan().target}"
            } else {
              "适用 \{plan().target}"
            }
          }, fn() { Green })
          |> ignore
        })
        |> ignore
        st.box(fn(box) {
          box
          .ground(Is(c_console))
          .edge(Is((1.0, c_border)))
          .radius(Is(6.0))
          .padding(Is((14.0, 12.0, 14.0, 12.0)))
          .gap(Is(6.0))
          |> ignore
          box.each(fn() { plan().mappings }, fn(x) { x.to }, fn(l, mp) {
            row(l, 6.0, fn(line) {
              text_by(line, fn() { mp.get().from }, size=12.0, ink=c_console_text).shrink(Is(1.0)).cuts(Is(true)) |> ignore
              text(line, "→", size=12.0, ink=c_accent) |> ignore
              text_by(line, fn() { mp.get().to }, size=12.0, ink=c_console_text).shrink(Is(1.0)).cuts(Is(true)) |> ignore
              text_by(line, fn() { "\{mp.get().files} 个文件" }, size=11.0, ink=c_muted) |> ignore
            })
            |> ignore
          })
          text_by(box, fn() {
            let x = plan()
            if x.conflicts.is_empty() {
              let up = if x.replaces == "" { "" } else { " · 将更新已安装的同名 Mod" }
              "✓ 无文件冲突 · 共 \{x.files} 个文件 · 安装前将自动快照存档并记录 Manifest 清单\{up}"
            } else {
              "⚠ \{x.conflicts.length()} 处与手动安装的文件冲突：\{x.conflicts.join("、")}（覆盖时原文件移到 .bunker/replaced）"
            }
          }, size=12.0, ink_by=fn() { if plan().conflicts.is_empty() { c_green } else { c_amber } })
          |> ignore
          box.show(fn() { !plan().skipped.is_empty() || !plan().warnings.is_empty() }, fn(sk) {
            text_by(sk, fn() {
              let x = plan()
              let w = x.warnings.join("；")
              let k = if x.skipped.is_empty() { "" } else { "未安装：\{x.skipped.join("、")}" }
              if w == "" { k } else if k == "" { w } else { "\{w}；\{k}" }
            }, size=11.5, ink=c_muted)
            |> ignore
          })
        })
        |> ignore
      }
    })
    row(b, 10.0, fn(f) {
      text(f, "也支持粘贴 GitHub Release / sp-mod 链接", size=11.5, ink=c_muted).grow(Is(1.0)) |> ignore
      f.show(fn() { s.install_step.get() == 2 }, fn(x) {
        x.row(Is(true)).gap(Is(10.0)) |> ignore
        button(x, "上一步", fn() { s.drop_plan() }) |> ignore
        button(x, "确认安装", fn() {
          match s.plan.untracked() {
            Some(pl) => s.install(!pl.conflicts.is_empty())
            None => ()
          }
        }, tone=Primary, label_by=fn() {
          match (s.busy.get(), s.plan.get()) {
            ("installing", _) => "正在安装…"
            (_, Some(pl)) if !pl.conflicts.is_empty() => "覆盖安装"
            _ => "确认安装"
          }
        }, disabled=fn() { s.busy.get() == "installing" })
        |> ignore
      }, along_row=true)
    })
    |> ignore
  })
  |> ignore
}
