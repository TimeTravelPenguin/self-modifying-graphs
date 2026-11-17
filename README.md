# Self-Modifying Graphs

This is a personal research project that looks at how graphs can evolve within an
environment, subject to constraints.

Graph vertices lie on a lattice. At each timestep, each node collects some amount of
resources from the environment. The amount collected is proportional to how much is
currently located at that vertex.

The environment diffuses its resources over time, as described by the following PDE:

```math
\frac{\partial R}{\partial t} = D\nabla^2 R(x, y, t) + S(x, y, t) - U(x, y, t)
```

where:

- $R(x, y, t)$ is the resource quantity at position (x, y) and time $t$.
- $D$ is the diffusion coefficient. Higher values result in faster diffusion.
- $S(x, y, t)$ is how much resource is returned to the environment, such as through an
  external intake.
- $U(x, y, t)$ is how much resource is extracted by the vertex at $(x, y)$ and time $t$.

We define $Q(x, y, t)$, which denotes how much resource is collected and stored by a
vertex at $(x, y)$ and time $t$, as:

```math
Q(x, y, t) = \eta(x, y, t) U(x, y, t)
```

where:

- $\eta(x, y, t)$ described the efficiency of resource extraction for the vertex at
  $(x, y)$ and time $t$. That is, a resource may extract some amount $x$, but only retain
  some proportion $0 \leq r \leq 1$. Thus, an amount $(1 - r) x$ is consumed/lost as a
  cost for resource extraction.

In this simulations, we describe $U$ with saturated scaling:

```math
U(x, y, t) = \min\left( R(x, y, t), \Delta t \frac{q_{\text{max}} R(x, y, t)}{K + R(x, y, t)} \right)
```

where $q_{\text{max}}$ is the maximum allowed extraction, and $K$ controls how much is
extracted when the resource concentration is low. $K$ is called the
_half-saturation constant_ and is defined as the concentration of resource at which the
extraction rate is half of the maximum extraction rate.
