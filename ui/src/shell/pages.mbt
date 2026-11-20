// 存档快照 (snapshots of the profiles: taken with a note, rolled back to, deleted) and 配置编辑 (a mod's JSON/JSONC or
// BepInEx .cfg: read, checked as it is written, saved with its old text kept as .bak).

///|
fn snapshots_page(v : @view.View, s : @model.Store) -> Unit {
  v.padding(Is((22.0, 18.0, 22.0, 18.0))).gap(Is(16.0)) |> ignore
  let note = @ui.Field::new()
  row(v, 10.0, fn(t) {
    line_field(t, note, "备注（例如：打 Labs 之前）", width=360.0, label="快照备注", submitted=fn(n) {
      s.snapshot(n)
      note.set("")
    })
    |> ignore
    button(t, "立即备份", fn() {
      s.snapshot(note.text())
      note.set("")
    }, tone=Primary, disabled=fn() { s.current.get() == "" })
    |> ignore
    spacer(t)
    text_by(t, fn() {
      let n = s.snapshots.get().length()
      let auto = s.snapshots.get().filter(fn(x) { x.kind != "manual" }).length()
      "共 \{n} 份快照，其中 \{auto} 份为自动备份"
    }, size=12.0, ink=c_text2)
    |> ignore
  })
  |> ignore
  v.box(fn(tb) {
    tb
    .grow(Is(1.0))
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .clips(Is(true))
    |> ignore
    tb.box(fn(sv) {
      sv.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).scrolls(@view.Scroll::new(), fn(list) {
      list.grow(Is(1.0)) |> ignore
      list.each(fn() { s.snapshots.get() }, fn(x) { x.id }, fn(r, snap) {
        table_row(r)
        r.height(Is(Some(68.0))).edge(Is((0.0, 0L))) |> ignore
        hover_ground(r, 0L, 0xFFFFFF06L)
        let confirm = @signals.signal(false)
        r.box(fn(c) {
          c.width(Is(Some(120.0))).gap(Is(3.0)) |> ignore
          text_by(c, fn() { s.moment(snap.get().created_at) }, size=13.5, weight=700) |> ignore
          text_by(c, fn() { @model.bytes_text(snap.get().bytes) }, size=11.0, ink=c_muted) |> ignore
        })
        |> ignore
        r.box(fn(c) {
          c.grow(Is(1.0)).shrink(Is(1.0)).gap(Is(4.0)) |> ignore
          text_by(c, fn() {
            let x = snap.get()
            if x.note == "" {
              kind_words(x.kind)
            } else {
              x.note
            }
          }, size=13.0, weight=600)
          .cuts(Is(true))
          |> ignore
          text_by(c, fn() {
            let x = snap.get()
            let who = if x.characters.is_empty() { "无角色" } else { x.characters.join("、") }
            "\{x.files} 个存档文件 · \{who}"
          }, size=11.5, ink=c_text2)
          .cuts(Is(true))
          |> ignore
        })
        |> ignore
        r.box(fn(c) {
          c.width(Is(Some(130.0))).row(Is(true)) |> ignore
          tag_by(c, fn() { kind_words(snap.get().kind) }, fn() { if snap.get().kind == "manual" { Gold } else { Muted } }) |> ignore
        })
        |> ignore
        r.box(fn(c) {
          c.width(Is(Some(190.0))).row(Is(true)).gap(Is(8.0)) |> ignore
          spacer(c)
          button(c, "回滚", fn() {
            if confirm.untracked() {
              confirm.set(false)
              s.restore(snap.untracked().id)
            } else {
              confirm.set(true)
            }
          }, small=true, label_by=fn() { if confirm.get() { "确认回滚" } else { "回滚到此" } }, disabled=fn() {
            let st = s.server.get().state
            st == "running" || st == "starting" || st == "external"
          })
          |> ignore
          button(c, "删除", fn() { s.delete_snapshot(snap.untracked().id) }, small=true, tone=Danger) |> ignore
        })
        |> ignore
      })
      list.show(fn() { s.snapshots.get().is_empty() }, fn(e) {
        e.padding(Is((20.0, 24.0, 20.0, 24.0))) |> ignore
        text(e, "还没有快照：点「立即备份」，或安装 Mod 时会自动备份", size=12.5, ink=c_muted) |> ignore
      })
    }) |> ignore
    })
    |> ignore
  })
  |> ignore
  text_by(v, fn() {
    match s.server.get().state {
      "running" | "starting" | "external" => "服务端运行中不能回滚（它会用内存里的存档覆盖回滚结果）：先停止服务端"
      _ => "回滚前会自动把当前存档再备份一份，回滚可撤销"
    }
  }, size=11.5, ink=c_muted)
  |> ignore
}

///|
fn config_page(v : @view.View, s : @model.Store) -> Unit {
  v.padding(Is((22.0, 18.0, 22.0, 18.0))).gap(Is(14.0)).row(Is(true)) |> ignore
  let editor = @ui.Field::new()
  let dirty = @signals.signal(false)
  // the list of files
  v.box(fn(l) {
    l
    .width(Is(Some(330.0)))
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .clips(Is(true))
    |> ignore
    l.box(fn(h) {
      h.padding(Is((16.0, 14.0, 16.0, 12.0))) |> ignore
      text_by(h, fn() { "配置文件（\{s.configs.get().length()}）" }, size=14.0, weight=700) |> ignore
    })
    |> ignore
    l.box(fn(line) { line.height(Is(Some(1.0))).ground(Is(c_border)) |> ignore }) |> ignore
    l.box(fn(sv) {
      sv.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).scrolls(@view.Scroll::new(), fn(list) {
      list.grow(Is(1.0)).padding(Is((8.0, 8.0, 8.0, 8.0))).gap(Is(2.0)) |> ignore
      list.each(fn() { s.configs.get() }, fn(c) { c.path }, fn(r, cf) {
        @headless.button(r, fn() { cf.get().path }, fn(_) {
          dirty.set(false)
          s.open_config(cf.untracked().path)
        }, fn(b, hb) {
          let cur = fn() { s.config_path.get() == cf.get().path }
          b
          .height(Is(Some(48.0)))
          .padding(Is((10.0, 7.0, 10.0, 7.0)))
          .radius(Is(6.0))
          .gap(Is(2.0))
          .cursor(Is(Some(Hand)))
          .ground(By(fn() { if cur() { c_accent_soft } else if hb.hovered() { c_hover } else { 0L } }))
          |> ignore
          text_by(b, fn() {
            let p = cf.get().path
            match p.rev_find("/") {
              Some(i) => p[i + 1:].to_owned()
              None => p
            }
          }, size=12.5, weight=600, ink_by=fn() { if cur() { c_accent } else { c_text } })
          .cuts(Is(true))
          |> ignore
          text_by(b, fn() { "\{cf.get().owner} · \{cf.get().kind.to_upper()} · \{@model.bytes_text(cf.get().bytes)}" }, size=11.0, ink=c_muted)
          .cuts(Is(true))
          |> ignore
        })
        |> ignore
      })
      list.show(fn() { s.configs.get().is_empty() }, fn(e) {
        text(e, "没有找到 Mod 配置文件", size=12.0, ink=c_muted) |> ignore
      })
    }) |> ignore
    })
    |> ignore
  })
  |> ignore
  // the editor
  v.box(fn(ed) {
    ed
    .grow(Is(1.0))
    .basis(Is(Some(0.0)))
    .shrink(Is(1.0))
    .min_width(Is(Some(0.0)))
    .min_height(Is(Some(0.0)))
    .clips(Is(true))
    .ground(Is(c_panel))
    .edge(Is((1.0, c_border)))
    .radius(Is(10.0))
    .padding(Is((16.0, 14.0, 16.0, 14.0)))
    .gap(Is(10.0))
    |> ignore
    ed.reacts(fn() { s.config_text.get() }, fn(t) {
      editor.set(t)
      dirty.set(false)
    })
    row(ed, 8.0, fn(h) {
      text_by(h, fn() {
        let p = s.config_path.get()
        if p == "" {
          "选择左侧的配置文件"
        } else {
          p
        }
      }, size=13.0, weight=700)
      .grow(Is(1.0))
      .cuts(Is(true))
      |> ignore
      tag_by(h, fn() { if dirty.get() { "未保存" } else { "" } }, fn() { Amber }) |> ignore
    })
    |> ignore
    ed.box(fn(fr) {
      // (as tall as the editor's room, never its words': a long file scrolls inside it, its foot's buttons kept in sight)
      fr
      .grow(Is(1.0))
      .basis(Is(Some(0.0)))
      .min_height(Is(Some(0.0)))
      .min_width(Is(Some(0.0)))
      .clips(Is(true))
      .ground(Is(c_console))
      .edge(Is((1.0, c_border)))
      .radius(Is(6.0))
      .padding(Is((12.0, 10.0, 12.0, 10.0)))
      .hides(By(fn() { s.config_path.get() == "" }))
      |> ignore
      fr.box(fn(sv) {
      sv.grow(Is(1.0)).basis(Is(Some(0.0))).min_height(Is(Some(0.0))).scrolls(@view.Scroll::new(), fn(sc) {
        sc.grow(Is(1.0)) |> ignore
        @ui.field(
          sc,
          editor,
          font_size=12.5,
          min_lines=20,
          max_lines=100000,
          color=c_console_text,
          placeholder_color=c_muted,
          caret_color=c_accent,
          enter_sends=false,
          label="配置内容",
          edited=fn(t) {
            dirty.set(true)
            s.check_config(t)
          },
        )
        |> ignore
      }) |> ignore
    })
      |> ignore
    })
    |> ignore
    row(ed, 10.0, fn(f) {
      text_by(f, fn() {
        match s.config_check.get() {
          "" => ""
          "ok" => "✓ 语法正确"
          e => "✗ \{e}"
        }
      }, size=12.0, ink_by=fn() {
        if s.config_check.get() == "ok" {
          c_green
        } else {
          c_red
        }
      })
      .grow(Is(1.0))
      .shrink(Is(1.0))
      .cuts(Is(true))
      |> ignore
      button(f, "撤销修改", fn() {
        editor.set(s.config_text.untracked())
        dirty.set(false)
        s.config_check.set("")
      }, disabled=fn() { !dirty.get() })
      |> ignore
      button(f, "校验并保存", fn() { s.save_config(editor.text()) }, tone=Primary, disabled=fn() {
        s.config_path.get() == "" || !dirty.get()
      })
      |> ignore
    })
    |> ignore
  })
  |> ignore
}
