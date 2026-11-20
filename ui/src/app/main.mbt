///|
fn main {
  @ui.run_tree("Bunker", fn(root) {
    root
    .grow(@view.Is(1.0))
    .ground(@view.Is(0x111315FFL))
    .pad(@view.Is(24.0))
    .box(fn(card) {
      card.words(@view.Is("Bunker"), ink=0xC8A258FFL) |> ignore
    })
    |> ignore
  })
}
