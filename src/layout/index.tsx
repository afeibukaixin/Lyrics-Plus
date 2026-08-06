import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";

function IndexLayout() {
  const navigate = useNavigate();

  useEffect(() => {
    const openSettings = (event: KeyboardEvent) => {
      if (!event.metaKey || event.code !== "Comma") return;
      event.preventDefault();
      navigate("/settings");
    };

    window.addEventListener("keydown", openSettings);
    return () => window.removeEventListener("keydown", openSettings);
  }, [navigate]);

  return (
    <>
      <Outlet />
    </>
  );
}

export default IndexLayout;
