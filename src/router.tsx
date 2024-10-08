import { createBrowserRouter } from "react-router-dom";

const router = createBrowserRouter([
  // Auth routes
  {
    path: "/",
    lazy: async () => ({
      Component: (await import("./pages/Start")).default,
    }),
  },
  {
    path: "/create",
    lazy: async () => ({
      Component: (await import("./pages/Create")).default,
    }),
  },
  {
    path: "/connect",
    lazy: async () => ({
      Component: (await import("./pages/Connect")).default,
    }),
  },
]);

export default router;
