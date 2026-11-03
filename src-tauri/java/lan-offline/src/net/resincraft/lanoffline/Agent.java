package net.resincraft.lanoffline;

import java.lang.instrument.Instrumentation;
import java.lang.reflect.Method;

/**
 * Lets offline players join this game's Open to LAN world. Minecraft always runs a LAN world in online mode; this turns
 * the account check off on the game's own server once it exists. The launcher adds it with -javaagent only when the
 * host chose it, and reads the lines it prints. It needs Minecraft's real names (26.1 and newer); older versions say so
 * and keep online mode.
 */
public final class Agent {
    private Agent() {}

    public static void premain(String args, Instrumentation inst) {
        Thread t = new Thread(() -> watch(inst), "ResinCraft LAN offline");
        t.setDaemon(true);
        t.start();
    }

    private static Class<?> find(Instrumentation inst, String name) {
        // Loaders like Fabric's keep the game out of the system class loader, so look among what's already loaded.
        for (Class<?> c : inst.getAllLoadedClasses()) {
            if (c.getName().equals(name)) {
                return c;
            }
        }
        return null;
    }

    private static void watch(Instrumentation inst) {
        Class<?> minecraft = null;
        Method getInstance = null;
        Method getServer = null;
        while (true) {
            try {
                Thread.sleep(1000);
            } catch (InterruptedException e) {
                return;
            }
            try {
                if (minecraft == null) {
                    minecraft = find(inst, "net.minecraft.client.Minecraft");
                    if (minecraft == null) {
                        continue;
                    }
                    getInstance = minecraft.getMethod("getInstance");
                    getServer = minecraft.getMethod("getSingleplayerServer");
                }
                Object client = getInstance.invoke(null);
                Object server = client == null ? null : getServer.invoke(client);
                if (server == null) {
                    continue;
                }
                // Each world starts its server with the check on, so this runs again for every world opened.
                if ((Boolean) server.getClass().getMethod("usesAuthentication").invoke(server)) {
                    server.getClass().getMethod("setUsesAuthentication", boolean.class).invoke(server, false);
                    System.out.println("[ResinCraft] LAN offline on");
                }
            } catch (NoSuchMethodException e) {
                System.out.println("[ResinCraft] LAN offline unsupported");
                return;
            } catch (Throwable e) {
                // The game is between worlds or still starting: try again next time.
            }
        }
    }
}
