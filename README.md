# Self-Modifying Graphs

> [!IMPORTANT]
> This project is still a work in progress. Additional information, including build
> instructions, will be added when the project is in a more complete state.

This is a personal research project that looks at how graphs can evolve within an
environment, subject to constraints.

Graph vertices lie on a lattice. At each timestep, each node collects some amount of
resources from the environment. The environment spreads resources out across the space.
Vertices may spawn new, neighbouring vertices with a connecting edge, or even connect an
edge to nearby vertices that it does not share an edge with. Edges may also be dropped.

The goal of this simulation is to explore the behaviour of graphs that attempt to optimise
themselves to an environment, constrained by some set of rules.

After completing the implementation of the basic simulation, the project will be extended
to add genetic, selection-based evolution to produce graphs through genome rules.

## Understanding The Simulation

The simulation can loosely be thought of as a seed in nature. Planted in nutrient-rich
soil, a seed will grow and spawn children. In this analogy, the children are new seeds
that are connected to the roots of the parent. Thus, the parent is able to provide
resources to the seed, while the seed is also able to collect its own resources.

If a seed or parent finds itself in a region that is nutrient-poor, it may survive and
reproduce if it is provided enough energy from its parent and other connected neighbours.

As new seeds are spawned, it may become too costly to pass energy through several
neighbours, and so new connections may be created to shortcut long paths. Additionally, if
a path eventually sees less traffic, it may be removed to conserve energy.

This is a simple system of dynamics that involves collecting and searching for resources
while maintaining as many vertices (seeds/plants/etc.) as possible.

Once this concept is more deeply explored, probabilistic or heuristic rules may be
replaced with genomes to dictate behaviour. It may also be possible to allow for several
species to compete—though this is not something that is yet being considered.

## Simulation Details

### Resource Environment

The environment diffuses its resources over time, as described by the following PDE:

```math
\frac{\partial R}{\partial t} = D\nabla^2 R(x, y, t) + S(x, y, t) - U(x, y, t)
```

where:

- $R(x, y, t)$ is the resource quantity at position $(x, y)$ and time $t$.
- $D$ is the diffusion coefficient. Higher values result in faster diffusion.
- $S(x, y, t)$ is how much resource is returned to the environment, such as through an
  external intake.
- $U(x, y, t)$ is how much resource is extracted by the vertex at $(x, y)$ and time $t$.

> [!NOTE]
> Future work will look into the case where $D = D(x, y, t)$ or where diffusion is variant
> over some topology. For example, diffusion may be higher when moving into lower regions.
>
> Similar considerations can also be given to graph edges, resulting in edges transferring
> energy between vertices at different rates when moving resources to vertices higher
> regions.

### Vertex Resource Collection

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

Some examples for $\eta$ include:

- $\eta(x, y, t) = r$, for $0 \leq r \leq 1$, which describes a constant, uniform
  efficacy across vertices.
- $\eta$ may decrease based on the number of neighbours within some radius. This results
  in vertices that become less efficicent as it becomes more croweded.
- $\eta$ may decrease as the number of edge connections increases, resulting in a
  similar behaviour as the previous example, except without consideration of spacial
  locality. Unlike the previous example, this case only considers _connected_ vertices.
- $\eta$ may decrease over time, proportional to the age of the vertex. This results in
  vertices that become less efficient over time. This may add pressure to the system if it
  cannot support the demand of other vertices.

In this simulation, we describe $U$ with saturated scaling:

```math
U(x, y, t) = \min\left( R(x, y, t), \Delta t \frac{q_{\text{max}} R(x, y, t)}{K + R(x, y, t)} \right)
```

where $q_{\text{max}}$ is the maximum allowed extraction, and $K$ controls how much is
extracted when the resource concentration is low. $K$ is called the
_half-saturation constant_ and is defined as the concentration of resource at which the
extraction rate is half of the maximum extraction rate.

### Edge Creation and Removal

> [!NOTE]
> This section is not yet implemented or formalised. Everything here is subject to change.

## Acknowledgement of AI Usage

AI has been used throughout this project to assist with optimisation, debugging,
maintaining documentation, including tests, and some of the graphical-related Bevy code.

AI has also been used to formalise mathematical ideas, correct or identify errors, and to
assist with locating resources related to my ideas.

All ideas are my own, and AI has been used solely as an aid to assist in research and as a
tool to maintain a correct and optimal codebase.

<table cellspacing="10">
  <tr>
    <td align="center" valign="middle" width="100">
      <a href="http://0rwx.com">
        <picture>
          <source
            srcset="https://raw.githubusercontent.com/NamtaoProductions/0rwx.com/refs/heads/main/static/AI-W-dark.svg"
            media="(prefers-color-scheme: dark)">
          <img
            src="https://raw.githubusercontent.com/NamtaoProductions/0rwx.com/refs/heads/main/static/AI-W-light.svg"
            alt="AI:W"
            width="100%">
        </picture>
      </a>
    </td>
    <td>
      <p>
        AI was only used to fix what already existed, or to refactor work into a more
        scalable design, with the only exceptions being for code related to UI and some
        visualisation code in Bevy. This Bevy code was still heavily instructed and
        reviewed.
      </p>
      <p>All ideas are my own.</p>
    </td>
  </tr>
</table>
