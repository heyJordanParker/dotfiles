# Positive recipes beat banned shapes: a Prompt shows the output it wants

A Prompt that names a shape to avoid plants that shape. Measured on our evals: a banned-shape
arm produced more of the banned shape than saying nothing, and a scope prohibition moved
codex scope from 0.88 to 0.75 by planting the act it named. A 20-item banned-vocabulary list
made replies worse, while one banned phrase plus one positive recipe moved codex
communication from 0.73 to 0.87, past the 11,000-word baseline. Chosen: a wrong output shape
is corrected with a positive recipe of what the output is, and at most one banned phrase.
Cost accepted: a recipe takes longer to write than a list of what not to do.
