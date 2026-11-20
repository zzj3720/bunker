// Bunker's window: its own title bar, the sidebar (the instance capsule and its popover, the four views, the server's
// card), the action bar (the character capsule and its popover, restarting the server, starting the game), and the
// view chosen. Every popover steps inside itself (adding an instance, an install's two steps) rather than opening
// another over it.

///|
pub fn app(root : @view.View, s : @model.Store) -> Unit {
  // (@sf's neutral dark theme: what its controls and, through parts.mbt's colours, the whole window are drawn in)
  @sf.set_dark(Some(true))
  @sf.set_accent("")
  @ui.set_tree_theme(@sf.ui_theme)
  root.ground(Is(c_app)).grow(Is(1.0)) |> ignore
  let inst_anchor : Ref[@view.View?] = Ref(None)
  let role_anchor : Ref[@view.View?] = Ref(None)
  let install_anchor : Ref[@view.View?] = Ref(None)
  let inst_pop = @headless.Popover::new(fn() { s.inst_open.get() }, fn(o) {
    s.inst_open.set(o)
    if !o {
      s.inst_step.set(1)
    }
  })
  let role_pop = @headless.Popover::new(fn() { s.role_open.get() }, fn(o) { s.role_open.set(o) })
  let install_pop = @headless.Popover::new(fn() { s.install_open.get() }, fn(o) {
    s.install_open.set(o)
    if !o {
      s.drop_plan()
    }
  })
  title_bar(root)
  root.box(fn(body) {
    body.row(Is(true)).grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).ground(Is(c_app)) |> ignore
    sidebar(body, s, inst_pop, inst_anchor)
    body.box(fn(main) {
      main.grow(Is(1.0)).basis(Is(Some(0.0))).min_width(Is(Some(0.0))).ground(Is(c_app)) |> ignore
      action_bar(main, s, role_pop, role_anchor)
      main.box(fn(content) {
        content.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).min_width(Is(Some(0.0))).clips(Is(true)) |> ignore
        content.pick(fn() { s.page.get() }, fn(v, page) {
          // (the page as tall as the window leaves it, never as its words: what is longer scrolls inside it)
          v.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).min_width(Is(Some(0.0))) |> ignore
          match page {
            Play => play_page(v, s)
            Mods => mods_page(v, s, install_anchor)
            Snapshots => snapshots_page(v, s)
            Config => config_page(v, s)
          }
        })
      })
      |> ignore
    })
    |> ignore
  })
  |> ignore
  // the popovers, over all of it, beside what opened each
  root.show(fn() { s.inst_open.get() }, fn(layer) {
    layer.over(Is(true)) |> ignore
    inst_pop.content(layer, fn() { "切换 SPT 实例" }, fn(p) {
      popover_panel(p, inst_anchor, 470.0, Start)
      instance_popover(p, s)
    })
    |> ignore
  })
  root.show(fn() { s.role_open.get() }, fn(layer) {
    layer.over(Is(true)) |> ignore
    role_pop.content(layer, fn() { "切换角色" }, fn(p) {
      popover_panel(p, role_anchor, 320.0, Start)
      role_popover(p, s)
    })
    |> ignore
  })
  root.show(fn() { s.install_open.get() }, fn(layer) {
    layer.over(Is(true)) |> ignore
    install_pop.content(layer, fn() { "安装新 Mod" }, fn(p) {
      popover_panel(p, install_anchor, 600.0, End)
      install_popover(p, s)
    })
    |> ignore
  })
  notice(root, s)
  // the window out of the way while the game runs, back as it ends (启动后最小化窗口)
  root.reacts(fn() { s.game.get() }, at_start=false, fn(g) {
    if s.minimize.untracked() {
      if g == "running" {
        @hostapi.window_act(1)
      } else if g == "exited" {
        @hostapi.window_show()
      }
    }
  })
}

///|
fn popover_panel(p : @view.View, anchor : Ref[@view.View?], w : Double, along : @view.Along) -> Unit {
  match anchor.val {
    Some(a) => p.beside(a.id(), side=Bottom, along~, offset=8.0, padding=8.0) |> ignore
    None => ()
  }
  // (@sf's small panel: its ground, edge, corners, padding and shadow)
  @sf.small_panel_node(p).width(Is(Some(w))) |> ignore
}

///|
fn title_bar(root : @view.View) -> Unit {
  root.box(fn(t) {
    t
    .row(Is(true))
    .lines_up(Is(Some(Middle)))
    .height(Is(Some(44.0)))
    .padding(Is((16.0, 0.0, 8.0, 0.0)))
    .ground(Is(c_titlebar))
    .edge(Is((0.0, 0L)))
    .gap(Is(10.0))
    .moves_window(Is(true))
    |> ignore
    t.box(fn(m) {
      m
      .width(Is(Some(24.0)))
      .height(Is(Some(24.0)))
      .radius(Is(6.0))
      .ground(Is(c_accent))
      |> ignore
      centred(m, fn(c) { c.box(fn(b) { b.words(Is("B"), ink=c_ink_on_accent, look=wl(13.0, weight=900)) |> ignore }) |> ignore })
    })
    |> ignore
    text(t, "Bunker", size=14.0, weight=700) |> ignore
    t.box(fn(b) {
      b.ground(Is(c_accent_soft)).edge(Is((1.0, c_border))).radius(Is(99.0)).padding(Is((7.0, 1.0, 7.0, 1.0))) |> ignore
      capsule_words(b, Is("SPT 开源启动器"), wl(11.0, weight=500), fn() { c_accent })
    })
    |> ignore
    spacer(t)
    t.box(fn(b) {
      b.edge(Is((1.0, c_border))).radius(Is(99.0)).padding(Is((12.0, 3.0, 12.0, 3.0))) |> ignore
      capsule_words(b, Is("mbui 原生 · 本地 core · 支持 SPT 3.11 / 4.0 / 4.1+"), wl(11.5), fn() { c_muted })
    })
    |> ignore
    spacer(t)
    let win_button = fn(label : String, glyph : String, act : Int) {
      @headless.button(t, fn() { label }, fn(_) { @hostapi.window_act(act) }, fn(v, b) {
        v
        .width(Is(Some(36.0)))
        .height(Is(Some(28.0)))
        .radius(Is(5.0))
        .ground(By(fn() { if b.hovered() { if act == 3 { c_red } else { c_hover } } else { 0L } }))
        |> ignore
        centred(v, fn(c) { c.box(fn(w) { w.words(Is(glyph), ink=c_text2, look=wl(12.0)) |> ignore }) |> ignore })
      })
      |> ignore
    }
    win_button("最小化", "─", 1)
    win_button("最大化", "□", 2)
    win_button("关闭", "✕", 3)
  })
  |> ignore
  root.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
}

///|
fn sidebar(body : @view.View, s : @model.Store, pop : @headless.Popover, anchor : Ref[@view.View?]) -> Unit {
  body.box(fn(sb) {
    sb
    .width(Is(Some(236.0)))
    .ground(Is(c_sidebar))
    .padding(Is((12.0, 14.0, 12.0, 14.0)))
    .gap(Is(14.0))
    |> ignore
    // the instance capsule: what opens the instance popover
    let cap = pop.trigger(sb, fn() { "切换 SPT 实例" }, fn(v, d) {
      v
      .ground(By(fn() { if d.open() || v.pointed() { c_hover } else { c_elevated } }))
      .edge(By(fn() { (1.0, if d.open() || v.pointed() { c_accent } else { c_border_strong }) }))
      .radius(Is(10.0))
      .padding(Is((12.0, 10.0, 12.0, 10.0)))
      .gap(Is(4.0))
      .cursor(Is(Some(Hand)))
      |> ignore
      row(v, 8.0, fn(r) {
        text_by(r, fn() {
          match s.current_inst() {
            Some(i) => i.name
            None => "未选择实例"
          }
        }, size=13.5, weight=700)
        .grow(Is(1.0))
        .cuts(Is(true))
        |> ignore
        tag_by(r, fn() {
          match s.current_inst() {
            Some(i) => i.spt_version
            None => ""
          }
        }, fn() { Green })
        |> ignore
      })
      |> ignore
      text_by(v, fn() {
        match s.current_inst() {
          Some(i) => "\{i.root} ▾"
          None => "点击添加 SPT 目录 ▾"
        }
      }, size=11.0, ink=c_muted)
      .cuts(Is(true))
      |> ignore
    })
    anchor.val = Some(cap)
    // the views
    sb.box(fn(nav) {
      nav.gap(Is(4.0)) |> ignore
      nav.box(fn(l) {
        l.padding(Is((10.0, 4.0, 10.0, 4.0))) |> ignore
        l.box(fn(w) { w.words(Is("工作台"), ink=c_muted, look=wl(11.0)) |> ignore }) |> ignore
      })
      |> ignore
      nav_item(nav, s, Play, "▶  启动与服务端", fn() {
        match s.server.get().state {
          "running" => "运行中"
          "starting" => "启动中"
          "external" => "外部运行"
          "failed" => "失败"
          _ => "就绪"
        }
      })
      nav_item(nav, s, Mods, "◫  Mod 管理", fn() { s.mods.get().filter(fn(m) { m.enabled }).length().to_string() })
      nav_item(nav, s, Snapshots, "◷  存档快照", fn() { s.snapshots.get().length().to_string() })
      nav_item(nav, s, Config, "⚙  配置编辑", fn() { "JSON/CFG" })
    })
    |> ignore
    spacer(sb)
    server_card(sb, s)
  })
  |> ignore
  body.box(fn(line) { line.width(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
}

///|
/// A view's item in the sidebar: @sf's sidebar row, its mark, its name and a pill at its end, chosen while its view shows.
fn nav_item(nav : @view.View, s : @model.Store, page : @model.Page, label : String, pill : () -> String) -> Unit {
  let (mark, name) = match label.split("  ").collect() {
    [m, n] => (m.to_owned(), n.to_owned())
    _ => ("", label)
  }
  @sf.nav_item_node(
    nav,
    Side,
    fn() { s.page.get() == page },
    fn() { label },
    fn(_) { s.page.set(page) },
    fn(m) { centred(m, fn(c) { c.box(fn(w) { w.words(Is(mark), look=wl(13.0)).ink(By(fn() { c_text2 })) |> ignore }) |> ignore }) },
    fn(t) { @sf.nav_title_node(t, fn() { name }) },
    end=fn(e) { pill_by(e, pill, fn() { s.page.get() == page }) },
  )
}

///|
fn server_card(sb : @view.View, s : @model.Store) -> Unit {
  sb.box(fn(c) {
    c
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .padding(Is((12.0, 10.0, 12.0, 10.0)))
    .gap(Is(6.0))
    |> ignore
    row(c, 6.0, fn(r) {
      dot(r, fn() { s.server.get().state })
      text(r, "SPT.Server", size=12.0).grow(Is(1.0)) |> ignore
      text_by(r, fn() { "127.0.0.1:\{s.server.get().port}" }, size=12.0, weight=600, ink_by=fn() {
        state_color(s.server.get().state)
      })
      |> ignore
    })
    |> ignore
    row(c, 6.0, fn(r) {
      text_by(r, fn() {
        match s.current_inst() {
          Some(i) if i.eft_version != "" => "EFT \{i.eft_version}"
          _ => "EFT 未知版本"
        }
      }, size=11.5, ink=c_muted)
      .grow(Is(1.0))
      |> ignore
      text_by(r, fn() { "\{s.server.get().errors} 报错" }, size=11.5, ink_by=fn() {
        if s.server.get().errors > 0 { c_red } else { c_muted }
      })
      |> ignore
    })
    |> ignore
  })
  |> ignore
}

///|
fn state_color(state : String) -> Int64 {
  match state {
    "running" | "external" => c_green
    "starting" | "stopping" => c_amber
    "failed" => c_red
    _ => c_muted
  }
}

///|
fn page_title(p : @model.Page) -> (String, String) {
  match p {
    Play => ("启动与服务端", "一键拉起服务端、自动认证角色并启动游戏，内置日志监控")
    Mods => ("Mod 管理", "服务端与客户端成对追踪，支持拖放安装、零残留禁用与依赖/汉化检测")
    Snapshots => ("存档快照", "一键备份角色存档、装 Mod 前自动快照、随时回滚")
    Config => ("配置编辑", "Mod 的 JSON / JSONC 与 BepInEx .cfg，保存前校验语法并保留 .bak")
  }
}

///|
fn action_bar(main : @view.View, s : @model.Store, pop : @headless.Popover, anchor : Ref[@view.View?]) -> Unit {
  main.box(fn(bar) {
    bar
    .row(Is(true))
    .lines_up(Is(Some(Middle)))
    .height(Is(Some(64.0)))
    .padding(Is((22.0, 0.0, 22.0, 0.0)))
    .ground(Is(c_actionbar))
    .gap(Is(10.0))
    |> ignore
    bar.box(fn(t) {
      t.grow(Is(1.0)).gap(Is(2.0)) |> ignore
      text_by(t, fn() { page_title(s.page.get()).0 }, size=17.0, weight=700) |> ignore
      text_by(t, fn() { page_title(s.page.get()).1 }, size=12.0, ink=c_text2) |> ignore
    })
    |> ignore
    // the character capsule
    let cap = pop.trigger(bar, fn() { "切换角色" }, fn(v, d) {
      v
      .row(Is(true))
      .lines_up(Is(Some(Middle)))
      .gap(Is(9.0))
      .ground(Is(c_elevated))
      .edge(By(fn() { (1.0, if d.open() || v.pointed() { c_accent } else { c_border_strong }) }))
      .radius(Is(99.0))
      .padding(Is((7.0, 5.0, 14.0, 5.0)))
      .cursor(Is(Some(Hand)))
      |> ignore
      v.box(fn(a) {
        a
        .width(Is(Some(28.0)))
        .height(Is(Some(28.0)))
        .radius(Is(14.0))
        .ground(Is(pal.chip))
        .edge(Is((1.0, c_border_strong)))
        |> ignore
        centred(a, fn(c) {
          c.box(fn(l) {
            l.words(By(fn() {
              match s.current_character() {
                Some(c) if c.side == "Usec" => "U"
                Some(_) => "B"
                None => "?"
              }
            }), ink=c_text, look=wl(11.0, weight=700))
            |> ignore
          })
          |> ignore
        })
      })
      |> ignore
      v.box(fn(t) {
        text_by(t, fn() {
          match s.current_character() {
            Some(c) => "\{c.shown()} (Lv.\{c.level})"
            None => "没有角色"
          }
        }, size=13.0, weight=700)
        |> ignore
        text_by(t, fn() {
          match s.current_character() {
            Some(c) => "\{edition_short(c.edition)} · \{@model.roubles_text(c.roubles)} ▾"
            None => "启动服务端后可创建 ▾"
          }
        }, size=11.0, ink=c_muted)
        |> ignore
      })
      |> ignore
    })
    anchor.val = Some(cap)
    button(bar, "仅重启服务端", fn() {
      match s.server.untracked().state {
        "running" | "starting" | "failed" => s.restart_server()
        _ => s.start_server()
      }
    }, label_by=fn() {
      match s.server.get().state {
        "running" | "starting" => "仅重启服务端"
        "stopping" => "正在停止…"
        "external" => "服务端外部运行中"
        _ => "仅启动服务端"
      }
    }, disabled=fn() {
      s.current.get() == "" || s.server.get().state == "stopping" || s.server.get().state == "external"
    })
    |> ignore
    button(bar, "▶ 一键启动游戏", fn() { s.launch() }, tone=Primary, label_by=fn() {
      match s.game.get() {
        "launching" => "正在启动…"
        "running" => "游戏运行中"
        _ => "▶ 一键启动游戏"
      }
    }, disabled=fn() { s.current.get() == "" || (s.game.get() != "idle" && s.game.get() != "exited") })
    |> ignore
  })
  |> ignore
  main.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
}

///|
fn edition_short(e : String) -> String {
  match e {
    "Edge Of Darkness" => "EoD"
    "Unheard" => "Unheard"
    "" => "未知版本"
    _ => e
  }
}

///|
fn instance_popover(p : @view.View, s : @model.Store) -> Unit {
  let path_field = @ui.Field::new()
  p.pick(fn() { s.inst_step.get() }, fn(v, step) {
    if step == 1 {
      // the instances
      row(v, 8.0, fn(h) {
        h.padding(Is((18.0, 14.0, 14.0, 14.0))) |> ignore
        text(h, "切换 SPT 实例", size=14.0, weight=700).grow(Is(1.0)) |> ignore
        button(h, "+ 添加实例", fn() { s.open_import() }, small=true) |> ignore
      })
      |> ignore
      v.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
      v.box(fn(list) {
        list.padding(Is((14.0, 12.0, 14.0, 12.0))).gap(Is(10.0)) |> ignore
        list.each(fn() { s.instances.get() }, fn(i) { i.root }, fn(r, inst) {
          @headless.button(r, fn() { inst.get().name }, fn(_) { s.select_instance(inst.untracked().root) }, fn(b, hb) {
            let cur = fn() { inst.get().root == s.current.get() }
            b
            .row(Is(true))
            .lines_up(Is(Some(Middle)))
            .height(Is(Some(64.0)))
            .padding(Is((16.0, 0.0, 14.0, 0.0)))
            .radius(Is(8.0))
            .gap(Is(10.0))
            .cursor(Is(Some(Hand)))
            .ground(By(fn() { if cur() { c_selected } else if hb.hovered() { c_hover } else { c_elevated } }))
            .edge(By(fn() { (1.0, if cur() { c_accent } else { c_border }) }))
            |> ignore
            b.box(fn(t) {
              t.grow(Is(1.0)).gap(Is(3.0)) |> ignore
              text_by(t, fn() { inst.get().name }, size=13.5, weight=700) |> ignore
              text_by(t, fn() {
                let i = inst.get()
                "\{i.root} · \{i.server_mods} Mods · \{i.layout_text()}"
              }, size=11.5, ink=c_text2)
              .cuts(Is(true))
              |> ignore
            })
            |> ignore
            tag_by(b, fn() { if cur() { "当前" } else { inst.get().spt_version } }, fn() {
              if cur() {
                Gold
              } else if inst.get().ok {
                Muted
              } else {
                Red
              }
            })
            |> ignore
          })
          |> ignore
        })
        list.show(fn() { s.instances.get().is_empty() }, fn(e) {
          text(e, "还没有实例：点「+ 添加实例」导入一个 SPT 目录", size=12.0, ink=c_muted) |> ignore
        })
      })
      |> ignore
      v.box(fn(f) {
        f.padding(Is((18.0, 10.0, 18.0, 14.0))) |> ignore
        text(f, "自动识别 3.11 / 4.0 (SPT/) 与 4.1+ (SPT_Runtime/)", size=11.5, ink=c_muted) |> ignore
      })
      |> ignore
    } else {
      // adding one: the same popover, stepped in
      row(v, 6.0, fn(h) {
        h.padding(Is((18.0, 14.0, 14.0, 14.0))) |> ignore
        button(h, "‹ 实例列表", fn() {
          s.inst_step.set(1)
          s.probe.set(None)
        }, tone=Ghost, small=true)
        |> ignore
        text(h, "导入已有 SPT 目录", size=14.0, weight=700) |> ignore
      })
      |> ignore
      v.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
      v.box(fn(b) {
        b.padding(Is((18.0, 14.0, 18.0, 16.0))).gap(Is(10.0)) |> ignore
        text(b, "选择包含 EscapeFromTarkov.exe 的文件夹：", size=12.0, ink=c_text2) |> ignore
        line_field(b, path_field, "例如 C:\\Games\\SPT-4.1.6", label="SPT 目录", edited=fn(t) { s.probe_path(t) }, submitted=fn(
          t,
        ) {
          match s.probe.untracked() {
            Some(i) if i.ok => s.add_instance(t)
            _ => s.probe_path(t)
          }
        })
        |> ignore
        // what the path holds, checked as it is typed
        b.show(fn() { s.probe.get() is Some(_) }, fn(r) {
          r.gap(Is(6.0)) |> ignore
          let check = fn(ok : () -> Bool, words : () -> String) {
            row(r, 8.0, fn(l) {
              text(l, "检测：", size=12.0, ink=c_muted) |> ignore
              text_by(l, fn() { if ok() { "✓" } else { "✗" } }, size=12.0, weight=700, ink_by=fn() {
                if ok() { c_green } else { c_red }
              })
              |> ignore
              text_by(l, words, size=12.0, ink=c_text) |> ignore
            })
            |> ignore
          }
          let probe = fn() {
            match s.probe.get() {
              Some(i) => i
              None => @model.Inst::{
                root: "",
                name: "",
                layout: "",
                server_rel: "",
                spt_version: "",
                eft_version: "",
                bepinex_version: "",
                server_mods: 0,
                client_mods: 0,
                profiles: 0,
                managed: 0,
                port: 0,
                ok: false,
                current: false,
                problems: [],
              }
            }
          }
          check(fn() { probe().spt_version != "" || probe().ok }, fn() {
            let i = probe()
            if i.ok {
              let exe = if i.server_rel == "" { "SPT.Server.exe" } else { "\{i.server_rel}/SPT.Server.exe" }
              "\{exe} (\{i.spt_version})"
            } else {
              i.problems.join("；")
            }
          })
          check(fn() { probe().bepinex_version != "" }, fn() {
            let i = probe()
            if i.bepinex_version != "" {
              "BepInEx \{i.bepinex_version} · \{i.client_mods} 个已装客户端 Mod"
            } else {
              "没有找到 BepInEx"
            }
          })
        })
        // what was found where SPT usually is
        b.show(fn() { !s.found.get().is_empty() && s.probe.get() is None }, fn(r) {
          r.gap(Is(6.0)) |> ignore
          text(r, "在常见位置找到：", size=11.5, ink=c_muted) |> ignore
          r.each(fn() { s.found.get() }, fn(i) { i.root }, fn(x, inst) {
            button(x, "", fn() {
              let root = inst.untracked().root
              path_field.set(root)
              s.probe_path(root)
            }, small=true, label_by=fn() { "\{inst.get().root}  ·  SPT \{inst.get().spt_version}" })
            |> ignore
          })
        })
        row(b, 10.0, fn(f) {
          f.margin(Is((0.0, 4.0, 0.0, 0.0))) |> ignore
          spacer(f)
          button(f, "返回", fn() {
            s.inst_step.set(1)
            s.probe.set(None)
          })
          |> ignore
          button(f, "确认导入", fn() { s.add_instance(path_field.text()) }, tone=Primary, disabled=fn() {
            match s.probe.get() {
              Some(i) => !i.ok
              None => true
            }
          })
          |> ignore
        })
        |> ignore
      })
      |> ignore
    }
  })
}

///|
fn role_popover(p : @view.View, s : @model.Store) -> Unit {
  row(p, 8.0, fn(h) {
    h.padding(Is((16.0, 12.0, 16.0, 12.0))) |> ignore
    text(h, "选择角色", size=14.0, weight=700).grow(Is(1.0)) |> ignore
    text_by(h, fn() { "\{s.characters.get().length()} 个存档" }, size=11.5, ink=c_muted) |> ignore
  })
  |> ignore
  p.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
  p.box(fn(list) {
    list.padding(Is((10.0, 10.0, 10.0, 10.0))).gap(Is(6.0)) |> ignore
    list.each(fn() { s.characters.get() }, fn(c) { c.username }, fn(r, ch) {
      @headless.button(r, fn() { ch.get().shown() }, fn(_) {
        s.character.set(ch.untracked().username)
        s.role_open.set(false)
      }, fn(b, hb) {
        let cur = fn() { ch.get().username == s.character.get() }
        b
        .row(Is(true))
        .lines_up(Is(Some(Middle)))
        .height(Is(Some(52.0)))
        .padding(Is((12.0, 0.0, 12.0, 0.0)))
        .radius(Is(8.0))
        .gap(Is(10.0))
        .cursor(Is(Some(Hand)))
        .ground(By(fn() { if cur() { c_selected } else if hb.hovered() { c_hover } else { c_elevated } }))
        .edge(By(fn() { (1.0, if cur() { c_accent } else { c_border }) }))
        |> ignore
        b.box(fn(t) {
          t.grow(Is(1.0)).gap(Is(2.0)) |> ignore
          text_by(t, fn() {
            let c = ch.get()
            if c.started {
              "\{c.shown()} (Lv.\{c.level})"
            } else {
              "\{c.username}（未进游戏创建角色）"
            }
          }, size=13.0, weight=700)
          |> ignore
          text_by(t, fn() {
            let c = ch.get()
            "\{c.side} · \{edition_short(c.edition)} · \{@model.roubles_text(c.roubles)}"
          }, size=11.0, ink=c_text2)
          |> ignore
        })
        |> ignore
        b.show(fn() { cur() }, fn(x) { tag(x, "当前", Gold) |> ignore })
      })
      |> ignore
    })
    list.show(fn() { s.characters.get().is_empty() }, fn(e) {
      text(e, "这个实例还没有存档：启动服务端后用 SPT 启动器创建角色", size=12.0, ink=c_muted) |> ignore
    })
  })
  |> ignore
}

///|
/// What was done or went wrong: @sf's toasts at the window's foot (one at a time: the latest notice).
fn notice(root : @view.View, s : @model.Store) -> Unit {
  @sf.toaster_node(
    root,
    fn() {
      match s.notice.get() {
        Some(n) => [@sf.toast("n\{n.n}", if n.bad { "⚠ \{n.text}" } else { n.text })]
        None => []
      }
    },
    fn(t) { if t.title.has_prefix("⚠") { 8000.0 } else { 4000.0 } },
    fn() { 24.0 },
    fn(_) {  },
    fn(id) {
      match s.notice.untracked() {
        Some(n) if "n\{n.n}" == id => s.notice.set(None)
        _ => ()
      }
    },
    w=560.0,
  )
  |> ignore
}
