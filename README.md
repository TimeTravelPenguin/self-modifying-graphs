# Self-modifying Graphs

This is a personal research project that looks at how graphs can evolve within an environment, subject to constraints.

Graph vertices lie on a lattice. At each time step, each node collects some amount of resources from the environment. 
The amount collected is proportional to how much is currently located at that vertex.

The environment diffuses its resources over time, as described by the following PDE:

```math
\frac{\partial R}{\partial t} = D\nabla^2 R(x, y, t) + S(x, y, t) + U(x, y, t)
```
