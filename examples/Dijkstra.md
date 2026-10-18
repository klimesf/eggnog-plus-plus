An example implementing Dijkstra's algorithm using eggnog++

Where `.` is a field where you can step on and `#` is where you cannot. `F` marks the finish, `S` marks the start.

```egg
let grid = input.to_char_grid()
let start = grid.find('S')
let goal = grid.find('F')
let seen = Set()
let queue = MinQueue(start, 0)

while (pos, dist) = queue.pop():
  if pos == goal:
    return dist
  if grid[pos] == '#' or !seen.add(pos):
    continue
  for p in grid.neighbors4(pos):
    queue.push(p, dist + 1)

panic("Cannot reach goal")
```
