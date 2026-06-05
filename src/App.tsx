import { useState } from "react";
import { ipc } from "@/lib/ipc";
import { Button } from "@/components/ui/button";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await ipc.greet(name));
  }

  return (
    <main className="grid min-h-screen place-items-center bg-background p-6">
      <div className="w-full max-w-md rounded-xl border bg-card p-8 shadow-sm">
        <h1 className="text-2xl font-semibold tracking-tight text-card-foreground">RMB</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Open-source business manager — walking skeleton.
        </p>

        <form
          className="mt-6 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            greet();
          }}
        >
          <label htmlFor="greet-input" className="sr-only">
            Your name
          </label>
          <input
            id="greet-input"
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="Enter a name…"
            className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
          <Button type="submit">Greet</Button>
        </form>

        {greetMsg && (
          <p className="mt-4 rounded-md bg-muted px-3 py-2 text-sm text-foreground">{greetMsg}</p>
        )}
      </div>
    </main>
  );
}

export default App;
