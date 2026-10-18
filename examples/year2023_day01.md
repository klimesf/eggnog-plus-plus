# Solution for AoC year 2023 day 01 in eggnog++.

## Naive solution

Part One:

```egg
let ans = 0
for line in input.lines():
  let a = line.chars().find(c => c.is_digit())
  let b = line.chars().find_last(c => c.is_digit())
  ans += a.digit() * 10 + b.digit()
ans

test """
     1abc2
     pqr3stu8vwx
     a1b2c3d4e5f
     treb7uchet
     """ == 142
```

Part Two:

```egg
let ans = 0
for line in input.lines():
  let nums = []
  while line.len() > 0:
    if line.starts_with("one") or line.starts_with("1"):
      nums.push(1)
    else if line.starts_with("two") or line.starts_with("2"):
      nums.push(2)
    else if line.starts_with("three") or line.starts_with("3"):
      nums.push(3)
    else if line.starts_with("four") or line.starts_with("4"):
      nums.push(4)
    else if line.starts_with("five") or line.starts_with("5"):
      nums.push(5)
    else if line.starts_with("six") or line.starts_with("6"):
      nums.push(6)
    else if line.starts_with("seven") or line.starts_with("7"):
      nums.push(7)
    else if line.starts_with("eight") or line.starts_with("8"):
      nums.push(8)
    else if line.starts_with("nine") or line.starts_with("9"):
      nums.push(9)
    line = line[1..]
  ans += nums.first() * 10 + nums.last()
ans

test """
     two1nine
     eightwothree
     abcone2threexyz
     xtwone3four
     4nineeightseven2
     zoneight234
     7pqrstsixteen
     """ == 281
```

## Shorter solution

Part One:

```egg
input.lines()
  .map(line => line.digits().first() * 10 + line.digits().last())
  .sum()

test """
     1abc2
     pqr3stu8vwx
     a1b2c3d4e5f
     treb7uchet
     """ == 142
```

Part Two:

```egg
let words = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"]

# The digit that starts at position i, or 0 if none does.
fun digit_at(line: string, i: int) -> int:
  if line[i].is_digit():
    return line[i].digit()
  if k = words.position(w => line.starts_with(w, at = i)):
    return k + 1
  0

let ans = 0
for line in input.lines():
  let digits = (0..line.len())
    .map(i => digit_at(line, i))
    .filter(d => d > 0)
    .to_list()
  ans += digits.first() * 10 + digits.last()
ans

test """
     two1nine
     eightwothree
     abcone2threexyz
     xtwone3four
     4nineeightseven2
     zoneight234
     7pqrstsixteen
     """ == 281
```
