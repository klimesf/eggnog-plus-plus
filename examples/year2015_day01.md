# Solution for AoC year 2015 day 01 in eggnog++.

## Naive solution

Part One:

```egg
let ans = 0
for c in input.chars():
  match c:
    '(': ans += 1
    ')': ans -= 1
    _: panic("unknown value")
ans

test "(())" == 0
test "(((" == 3
test "))(" == -1
test ")())())" == -3
```

Part Two:

```egg
let floor = 0
let pos = 1
for c in input.chars():
  match c:
    '(': floor += 1
    ')': floor -= 1
    _: panic("unknown value")
  if floor < 0:
    return pos
  pos += 1

panic("did not reach basement")

test ")" == 1
test "()())" == 5
```

## Counting solution

Part One:

```egg
input.count('(') - input.count(')')

test "(())" == 0
test "(((" == 3
test "))(" == -1
test ")())())" == -3
```

## Functional solution

Part One:

```egg
input.chars()
  .map(c => if c == '(' then 1 else -1)
  .sum()

test "(())" == 0
test "(((" == 3
test "))(" == -1
test ")())())" == -3
```

Part Two:

```egg
input.chars()
  .map(c => if c == '(' then 1 else -1)
  .scan(init = 0, (floor, step) => floor + step)
  .position(floor => floor < 0) + 1

test ")" == 1
test "()())" == 5
```
