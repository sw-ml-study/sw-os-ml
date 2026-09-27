# The dream, top down

Why MLOS exists, in five rungs, each one paragraph. Every rung links to
the document that owns its detail; nothing here is the source of truth
for anything, and that is the point of it. Read
[anatomy.md](anatomy.md) next for what has actually been built.

## The premise, in one breath

Every operating system since Unix manages memory by reacting. A page is
touched, it is missing, something else is thrown out on a guess about
the past. Inference does not need to guess. A transformer reads its
weights in the same order every token and grows its cache one block at a
time, so its future is known before it happens. MLOS is an operating
system that lets a workload hand over that future through a system call,
and replaces the guess with a plan. Measured against LRU on a real
model's shape, the plan reads 42 to 77 percent less where memory is
tight.[^band]

## Mission

Find out, by measurement, whether an operating system that is *told* the
future of machine-learning state beats one that guesses, and stop if it
does not. Everything in this repository is in service of that sentence.
The first four milestones built the mechanism a measurement needs; the
fourth gate ([g4-report.md](g4-report.md)) is the measurement, and its
report says what it does not show as loudly as what it does. A negative
result would have ended the project, and the plan said so in advance
([plan.md](plan.md#saga-mlos-nextuse-m3)).

## Vision

A distributed control plane for ML state. The host operating systems keep
owning what they are good at, drivers, filesystems, networking, GPU
runtimes, and MLOS owns the one thing none of them can express: which
weights, cache blocks, experts and activations should be resident where,
at what precision, at what cost, across GPU memory, RAM, SSD and the
network, for many sessions at once. Tiers are a cost graph, not a
ladder, because a peer's RAM over a fast link can be cheaper than local
disk. [architecture.md s.7.4](architecture.md#74-the-hostguest-boundary-what-mlos-does-not-own)
draws the boundary and [s.7.5](architecture.md#75-tiers-are-a-cost-graph-not-a-ladder)
the graph; [clustering.md](clustering.md) is how one instance becomes many.

## Requirements

A new kernel, not a fork, because the abstraction it presents is the
thesis and inheriting a page-based memory system would defeat it. Written
in Rust 2024 with `no_std` in the kernel and `unsafe` confined to the
hardware layer and the drivers, each block naming its invariant. It boots
only as a guest in a virtual machine, on two architectures under two
hypervisors each, so no vendor's quirks become load-bearing. Every
residency policy must be replayable on the host, against a recorded
trace, before it enters the kernel, and the kernel must reproduce the
host's numbers exactly. The full list, functional and non-functional, is
[PRD.md s.6](PRD.md#6-requirements); what was deliberately ruled out is
[s.7](PRD.md#7-explicit-non-goals).

## Objectives

Eight gates, each a demonstration rather than a quantity of code
([PRD.md s.5.1](PRD.md#51-the-proof-of-concept-gate-the-thing-we-are-building-toward)).
Four are met: it boots to a shell (G1), it holds an object table (G2),
touching a missing object raises a fault that names the layer rather than
an address (G3), and a policy told the future beats LRU and FIFO under an
identical budget (G4). The next four: one read serves many sessions
(G5), it degrades instead of dying (G6), it places objects in host
resources it does not drive (G7), and another machine is a provider
(G8). [status.md](status.md) is the ground truth for which are met and
what each cost.

## Plan

Eleven milestones, M0 to M10, one saga each, ending at a result rather
than a line count. M0 to M3 are done. M4 and M5 are the OS semantics
that decide whether "ML OS" is an architecture or a repackaging of
framework tricks, and they need no GPU at all. M6 onward reaches real
host resources, then distribution, heterogeneity and global scheduling,
with the hardware ML-MMU deliberately last so it accelerates what has
been shown to work. Sagas run in parallel on their own branches; the
plan is what keeps them from colliding. [plan.md](plan.md) has the
milestones and every saga's steps.

## Who, when, where

Software Wrighter LLC: one developer directing coding agents, working in
recorded steps so that a session with no memory of the last can pick up
where it left off. Started in mid-2026; M3 closed on 2026-09-27. Built
on an Apple Silicon Mac, with a Linux x86-64 machine next so that nothing
depends on the machine it was written on. The sibling repositories it
trades artifacts with are in [external-asks.md](external-asks.md).

[^band]: The advantage is a band of memory budgets near the model's
    per-token working set. Below it every reactive policy misses every
    access and knowing the future saves only what fits; above it
    everything fits and no policy differs. Where the band sits depends on
    the model and the serving regime, and the traces were derived from a
    real checkpoint's tensor inventory rather than recorded from an
    inference engine. [g4-report.md](g4-report.md) states all of this.
