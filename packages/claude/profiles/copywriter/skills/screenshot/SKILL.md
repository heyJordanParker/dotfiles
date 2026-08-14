---
name: screenshot
description: Capture a full-page screenshot of a web page through agent-browser, session closed in the same step. TRIGGER when a research or review step needs a page captured as an image — a competitor's landing page, a rendered design. DO NOT TRIGGER to read a page's text (browse) or to score a source (review-source).
---

# Screenshot

One Process — capture one page as an image through agent-browser.

## 1. Capture the page and close the session

Open the URL through agent-browser (`agent-browser skills get core`), wait for the page to load, screenshot the full page into the working folder, and close the session in this same step. An abandoned session is the known failure.

### A bot-walled page cannot be screenshotted
A page behind a bot wall renders no content to agent-browser, so it cannot be captured as an image — read its text with browse instead, record the gap in the working notes, and move on. Company sites rarely wall, so a competitor's own landing page normally captures cleanly.

Verification: the screenshot is saved in the working folder and the agent-browser session was closed in the same step; a bot-walled page was recorded as a text-only capture, not left as an abandoned session.
