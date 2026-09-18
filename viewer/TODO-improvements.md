# Replay Viewer — improvements queued for tomorrow (2026-08-08 EOD)

Requested by the owner; to be implemented by an agent **after review tomorrow** (confirm scope,
especially item 7 which was left incomplete). These build on the current viewer (`viewer/`, vanilla
TS canvas) with its Answer panel, Tools playground, and tool-call analytics panel.

1. **Board window → tabbed (Board | Tool-call analytics).** The board div should become a *window
   with two tabs*: one tab is the Board, the other is Tool-call analytics; switch between them in the
   same window (instead of the analytics being a separate always-visible panel).

2. **Reduce the board's vertical footprint + add collapsibles.** The board eats too much vertical
   space, so the question + legend are off-view by default. Shrink the board *a little* vertically
   (careful — over-shrinking hurts). **More importantly:** move the **question** into a *collapsible*
   section inside the Answer window; make the **model's answer text** collapsible too, and its
   **reasoning** — the same collapsible treatment the full prompt already has.

3. **Legend: reorder + prune.** The **faction-color** legend and the **referent / candidate /
   fetched-region** markers are the most important → put them **first**. Remove the **City** and
   **Unit** legend entries — those are drawn as sprites, so the legend rows are useless.

4. **"Model's answer text" == "Reasoning" — investigate.** These two fields always render *identical*
   text. Find out why. If there's an obvious bug, fix it; **if not, remove "Reasoning" as redundant.**

5. **Tools → a tab inside the Answer window.** Move the Tools playground into a *tab living in the
   same window as Answer*, so the user can run tools **while looking at the board** — currently their
   positioning makes it impossible to see the board and the tools at once.

6. **Coordinate labels on the board edge.** Make it easier to relate a visual tile to its Cartesian
   coords — label the row/column numbers along the map edges. Tricky with 2–3 digit numbers and
   spacing; needs a thoughtful layout (e.g. periodic ticks, or edge gutters).

7. **[INCOMPLETE — owner to finish the thought]** Another tab that, instead of showing the
   board/answer/questions, lets you **combine filters to select …** *(sentence was cut off in the
   request — likely a filter-builder view to compose/select a question subset by ANDing filters;
   confirm intent before building.)*

**Layout note:** items 1 and 5 both reorganize content into tabbed windows (Board|Analytics, and
Answer|Tools). Design them together so the two tabbed windows are visually consistent.
