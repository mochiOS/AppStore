const releases = [
  {
    version: "2.4.0",
    channel: "Internal QA",
    status: "Uploading",
  },
  {
    version: "2.3.8",
    channel: "Store review",
    status: "Waiting",
  },
  {
    version: "2.3.7",
    channel: "Phased rollout",
    status: "10%",
  },
];

export default function ReleasesPage() {
  return (
    <div className="space-y-6">
      <section className="rounded-[2rem] border border-zinc-200 bg-white p-6">
        <p className="text-xs font-medium uppercase tracking-[0.32em] text-zinc-500">Releases</p>
        <h3 className="mt-3 text-2xl font-semibold tracking-tight text-zinc-950">Release queue</h3>
      </section>

      <section className="grid gap-4">
        <div className="rounded-[2rem] border border-zinc-200 bg-white p-6">
          <div className="mt-5 space-y-3">
            {releases.map((release) => (
              <div
                key={release.version}
                className="flex flex-col gap-3 rounded-3xl border border-zinc-200 px-4 py-4 sm:flex-row sm:items-center sm:justify-between"
              >
                <div>
                  <p className="font-medium text-zinc-950">{release.version}</p>
                  <p className="text-sm text-zinc-500">{release.channel}</p>
                </div>
                <span className="rounded-full bg-zinc-100 px-3 py-1 text-xs font-medium text-zinc-700">
                  {release.status}
                </span>
              </div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
