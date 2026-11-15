#!/usr/bin/env python3
"""Download a year's Advent of Code inputs and puzzle texts for offline work.

Usage:
    AOC_SESSION=<token> AOC_CONTACT=<email or repo url> python3 fetch.py 2024

Writes input.txt, puzzle.md and answers.txt into yYEAR/dayDD/ next to this script.
Existing files are never re-downloaded, except a puzzle.md that only has
part 1 (re-run after solving part 1 online to get part 2's text).
All three are gitignored: AoC asks that inputs and puzzle texts not be shared.
"""

import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

DELAY_SECONDS = 2  # pause between requests, to go easy on the AoC servers


def fetch(url: str, session: str, contact: str) -> str | None:
    request = urllib.request.Request(url, headers={
        "Cookie": f"session={session}",
        "User-Agent": f"eggnog++ offline fetch script (contact: {contact})",
    })
    try:
        with urllib.request.urlopen(request) as response:
            return response.read().decode()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None  # the day doesn't exist (2025 has 12) or isn't out yet
        raise
    finally:
        time.sleep(DELAY_SECONDS)


def to_markdown(page: str) -> str:
    # The puzzle text is in one <article class="day-desc"> per part.
    articles = re.findall(r'<article class="day-desc">(.*?)</article>', page, re.DOTALL)
    html = "\n".join(articles)
    try:
        result = subprocess.run(["pandoc", "-f", "html", "-t", "gfm", "--wrap=none"],
                                input=html, capture_output=True, text=True, check=True)
        return result.stdout
    except FileNotFoundError:
        return html  # no pandoc: keep the HTML, which still reads fine


def main() -> None:
    if len(sys.argv) != 2 or not sys.argv[1].isdigit():
        sys.exit("usage: AOC_SESSION=<token> AOC_CONTACT=<contact> python3 fetch.py YEAR")
    year = int(sys.argv[1])
    session = os.environ.get("AOC_SESSION")
    contact = os.environ.get("AOC_CONTACT")
    if not session or not contact:
        sys.exit("set AOC_SESSION (the session cookie) and AOC_CONTACT (an email or repo url)")

    root = Path(__file__).parent / f"y{year}"
    for day in range(1, 26):
        folder = root / f"day{day:02}"
        url = f"https://adventofcode.com/{year}/day/{day}"

        puzzle = folder / "puzzle.md"
        answers = folder / "answers.txt"
        if not puzzle.exists() or not answers.exists() or "Part Two" not in puzzle.read_text():
            page = fetch(url, session, contact)
            if page is None:
                print(f"day {day}: not found, stopping")
                break
            folder.mkdir(parents=True, exist_ok=True)
            puzzle.write_text(to_markdown(page))
            # Solved parts show their answer below the text, outside the articles.
            found = re.findall(r"Your puzzle answer was <code>(.*?)</code>", page)
            answers.write_text("".join(f"part {i + 1}: {a}\n" for i, a in enumerate(found)))
            parts = 2 if "Part Two" in puzzle.read_text() else 1
            print(f"day {day}: puzzle text ({parts} part{'s' if parts == 2 else ''}), {len(found)} answers")

        input_file = folder / "input.txt"
        if not input_file.exists():
            text = fetch(f"{url}/input", session, contact)
            if text is None:
                print(f"day {day}: no input")
                continue
            input_file.write_text(text)
            print(f"day {day}: input")


if __name__ == "__main__":
    main()
