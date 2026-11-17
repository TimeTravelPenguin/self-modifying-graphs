# Self-Modifying Graphs

> [!IMPORTANT]
> This project is still a work in progress. Additional information, including build
> instructions, will be added when the project is in a more complete state.

This is a personal research project that looks at how graphs can evolve within an
environment, subject to constraints.

Graph vertices lie on a lattice. At each timestep, each node collects some amount of
resources from the environment. The amount collected is proportional to how much is
currently located at that vertex.

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
        AI was only used to fix what already existed, or to refactor work into a more scalable design.
      </p>
      <p>All ideas are my own.</p>
    </td>
  </tr>
</table>
