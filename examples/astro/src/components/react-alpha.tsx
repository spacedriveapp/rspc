import { initRspc, httpLink, wsLink } from "@oscartbeaumont-sd/rspc-client/v2";
import { createReactQueryHooks } from "@oscartbeaumont-sd/rspc-react/v2";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { useState } from "react";

// Export from Rust. Run `cargo run -p example-axum` to start server and export it!
import type { Procedures } from "../../../bindings";

const fetchQueryClient = new QueryClient();
const fetchClient = initRspc<Procedures>({
  links: [
    // loggerLink(),

    httpLink({
      url: "http://localhost:4000/rspc",

      // You can override the fetch function if required
      // fetch: (input, init) => fetch(input, { ...init, credentials: "include" }), // Include Cookies for cross-origin requests

      // Provide static custom headers
      // headers: {
      //   "x-demo": "abc",
      // },

      // Provide dynamic custom headers
      // headers: ({ op }) => ({
      //   "x-procedure-path": op.path,
      // }),
    }),
  ],
  // onError // TODO: Make sure this is still working
});

const wsQueryClient = new QueryClient();
const wsClient = initRspc<Procedures>({
  links: [
    // loggerLink(),

    wsLink({
      url: "ws://localhost:4000/rspc/ws",
    }),
  ],
});

// TODO: Allowing one of these to be used for multiple clients! -> Issue is with key mapper thing
// TODO: Right now we are abusing it not working so plz don't do use one of these with multiple clients in your own apps.
export const rspc = createReactQueryHooks<Procedures>(fetchClient);
// export const rspc2 = createReactQueryHooks<Procedures>(wsClient);

function Example({ name }: { name: string }) {
  const [rerenderProp, setRendererProp] = useState(Date.now().toString());
  const { data: version } = rspc.useQuery(["version"]);
  const { data: transformMe } = rspc.useQuery(["transformMe"]);
  const { data: echo } = rspc.useQuery(["echo", "Hello From Frontend!"]);
  const { mutate, isLoading } = rspc.useMutation("sendMsg");
  const { error } = rspc.useQuery(["error"], {
    retry: false,
  });

  return (
    <div
      style={{
        border: "black 1px solid",
      }}
    >
      <h1>{name}</h1>
      <p>Using rspc version: {version}</p>
      <p>Echo response: {echo}</p>
      <p>
        Error returned: {error?.code} {error?.message}
      </p>
      <p>Transformed Query: {transformMe}</p>
      <ExampleSubscription rerenderProp={rerenderProp} />
      <button onClick={() => setRendererProp(Date.now().toString())}>
        Rerender subscription
      </button>
      <button onClick={() => mutate("Hello!")} disabled={isLoading}>
        Send Msg!
      </button>
    </div>
  );
}

function ExampleSubscription({ rerenderProp }: { rerenderProp: string }) {
  const [i, setI] = useState(0);
  rspc.useSubscription(["pings"], {
    onData(msg) {
      setI((i) => i + 1);
    },
  });

  return (
    <p>
      Pings received: {i} {rerenderProp}
    </p>
  );
}

export default function App() {
  return (
    <React.StrictMode>
      <div
        style={{
          backgroundColor: "rgba(50, 205, 50, .5)",
        }}
      >
        <h1>React</h1>
        <QueryClientProvider client={fetchQueryClient} contextSharing={true}>
          <rspc.Provider client={fetchClient} queryClient={fetchQueryClient}>
            <Example name="Fetch Transport" />
          </rspc.Provider>
        </QueryClientProvider>
        <rspc.Provider client={wsClient} queryClient={wsQueryClient}>
          <QueryClientProvider client={wsQueryClient}>
            <Example name="Websocket Transport" />
          </QueryClientProvider>
        </rspc.Provider>
      </div>
    </React.StrictMode>
  );
}
