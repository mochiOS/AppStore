const apps = [
  {
    name: "Mochi Notes",
    platform: "iOS",
    version: "2.4.0",
    status: "Published",
  },
  {
    name: "Mochi Studio",
    platform: "macOS",
    version: "1.9.3",
    status: "In review",
  },
  {
    name: "Mochi Drive",
    platform: "iPadOS",
    version: "3.1.1",
    status: "Needs fix",
  },
];

export default function AppsPage() {
  return (
    <div className="space-y-6">
      <section className="rounded-[2rem] border border-zinc-200 bg-white p-6">
        <p className="text-xs font-medium uppercase tracking-[0.32em] text-zinc-500">Apps</p>
        <h3 className="mt-3 text-2xl font-semibold tracking-tight text-zinc-950">App list</h3>
      </section>

      <section className="grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
        {apps.map((app) => (
          <article key={app.name} className="rounded-[2rem] border border-zinc-200 bg-white p-6">
            <div className="flex items-start justify-between gap-4">
              <div>
                <h4 className="text-lg font-semibold tracking-tight text-zinc-950">{app.name}</h4>
                <p className="mt-1 text-sm text-zinc-500">
                  {app.platform} · {app.version}
                </p>
              </div>
              <span className="rounded-full bg-zinc-100 px-3 py-1 text-xs font-medium text-zinc-700">
                {app.status}
              </span>
            </div>
          </article>
        ))}
      </section>
    </div>
  );
}
