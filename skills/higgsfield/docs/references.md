# Higgsfield References

## How To Make an AI Animated Short Film (Full Workflow)
- **URL:** https://www.youtube.com/watch?v=zYPgz6sOy74
- **By:** Higgsfield AI (official) — 2026-04-09, 16:37, 226k views
- **Saved:** 2026-06-14 by Val

### Key Techniques
1. **Character consistency across scenes** — feed the *previous video clip* as `--image` input into every new Seedance generation. The model sees the prior frame and maintains the same character look.
2. **Model to use:** Seedance 2.0 (not Kling) for animated/character-driven scenes — better motion quality and consistency
3. **Prompting workflow:** Use Claude to turn rough scene ideas into cinematic Seedance 2.0 prompts
4. **Soul Cinema mode** — Higgsfield's character-consistent animation mode, works with Seedance 2.0

### Claude Skill for Seedance Prompts
- Dropbox link (from video desc): https://higgsfield.ai/s/seedance-2-0-higgsfieldai-PYAcMw
- Full prompts used: https://higgsfield.ai/s/seedance-2-0-higgsfieldai-cXSLlg

### Application to Coven Podcast
- Switch Nova/Sage/Charm podcast clips from `kling3_0` → `seedance_2_0`
- Use prior clip as reference image for next generation to maintain familiars' appearance
- Consider Soul Cinema mode for the speaking/listening character shots
