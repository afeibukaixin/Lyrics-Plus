import { Toaster as Sonner, type ToasterProps } from "sonner";

export function Toaster(props: ToasterProps) { return <Sonner theme="system" position="bottom-right" richColors closeButton {...props} />; }
