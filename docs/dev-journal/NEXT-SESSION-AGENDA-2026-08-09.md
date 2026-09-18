# Next-session agenda — owner's note, verbatim (written EOD 2026-08-09)

> Copied word-for-word from the owner's end-of-day message so it survives a terminal clear.
> This is the to-do list for the NEXT session. Working notes / analysis pointers are in
> `CONTINUITY-2026-08-09.md`; durable state is in `RESUME.md`.

---

I'm stopping for today. I want you to update all docs, merge any un merged work that's ready, and write in the continuity file if you haven't already.

Additionally there's a few things I want us to note:
1. We have to review tools again next time. Most importantly we need to get the LLM to use region_summary more, and get_tile less.
2. I am still skeptical of scan_grid and tomorrow we should try to devise some cheap experiment to see if it's pulling its worth. Maybe we don't need to do that and can deduce an answer from existing data. But I'm leaning towards it not being good. It gives less information than region_summary, which I suspect leads to more turns being taken.
3. Tool-Call Analytics should be removed from coexisting with the board in a tab and instead Tool-Call Analytics should be shown at the bottom of the Aggregated stats, including appearing in the Compare mode.
4. There's an argument to be made for list_units content and list_cities content to be included in the system prompt. I know this queers the difference between interactive and raw, but for most cases I think it would make sense? We can discuss this.
5. If constraint-site results in 3 tools being used 100% of the time in interactive-maxops then it makes look as if that question is nothing more than testing the LLM's ability to call those 3 tools, which makes it not the most edifying question. We need to analyze.
6. I want the Replay Viewer to have some analysis somewhere that shows often it happens that one of the tools is called multiple times in DIFFERENT turns for the same question (very important that it's in different turns).

Again, this list is stuff we're supposed to tackle tomorrow. I would like you to copy my message word for word in a last message .md mostly so I can read it myself if I clear this terminal. We good?
