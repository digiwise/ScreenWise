// Content-free high-frequency descendant-process sampler for short GUI trials.
// Default audit mode uses Toolhelp snapshots only; it does not inspect command lines, windows,
// captured content, the clipboard, audio devices, files outside its output
// directory, or the network.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

internal static partial class GuiProcessTreeMonitor
{
    private const uint SnapshotProcesses = 0x00000002;
    private static readonly IntPtr InvalidHandle = new IntPtr(-1);
    private static readonly HashSet<string> UnexpectedShells = new HashSet<string>(
        new[]
        {
            "powershell.exe", "pwsh.exe", "conhost.exe", "cmd.exe", "wscript.exe", "cscript.exe",
            "setx.exe", "where.exe"
        },
        StringComparer.OrdinalIgnoreCase);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    private struct ProcessEntry32
    {
        public uint dwSize;
        public uint cntUsage;
        public uint th32ProcessID;
        public IntPtr th32DefaultHeapID;
        public uint th32ModuleID;
        public uint cntThreads;
        public uint th32ParentProcessID;
        public int pcPriClassBase;
        public uint dwFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szExeFile;
    }

    private sealed class Entry
    {
        internal int Id;
        internal int ParentId;
        internal string Name;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint processId);

    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern bool Process32First(IntPtr snapshot, ref ProcessEntry32 entry);

    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern bool Process32Next(IntPtr snapshot, ref ProcessEntry32 entry);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private static List<Entry> Snapshot()
    {
        List<Entry> result = new List<Entry>();
        IntPtr snapshot = CreateToolhelp32Snapshot(SnapshotProcesses, 0);
        if (snapshot == InvalidHandle) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try
        {
            ProcessEntry32 native = new ProcessEntry32();
            native.dwSize = (uint)Marshal.SizeOf(typeof(ProcessEntry32));
            if (!Process32First(snapshot, ref native))
                throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            do
            {
                result.Add(new Entry
                {
                    Id = checked((int)native.th32ProcessID),
                    ParentId = checked((int)native.th32ParentProcessID),
                    Name = native.szExeFile ?? String.Empty
                });
            }
            while (Process32Next(snapshot, ref native));
            int error = Marshal.GetLastWin32Error();
            if (error != 18) // ERROR_NO_MORE_FILES is the only successful end of enumeration.
                throw new System.ComponentModel.Win32Exception(error);
            return result;
        }
        finally { CloseHandle(snapshot); }
    }

    private static string Escape(string value)
    {
        return value.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\r", "\\r").Replace("\n", "\\n");
    }

    private static bool IsUnexpectedShell(string name)
    {
        return UnexpectedShells.Contains(name ?? String.Empty);
    }

    private static int Run(int rootProcessId, string outputDirectory, int durationMilliseconds)
    {
        if (rootProcessId <= 0) throw new ArgumentException("invalid_root_process_id");
        if (String.IsNullOrWhiteSpace(outputDirectory) || !Path.IsPathRooted(outputDirectory))
            throw new ArgumentException("invalid_output_directory");
        outputDirectory = Path.GetFullPath(outputDirectory);
        if (!Directory.Exists(outputDirectory)) throw new DirectoryNotFoundException("output_directory_missing");

        string eventsPath = Path.Combine(outputDirectory, "descendant-process-starts.jsonl");
        string summaryPath = Path.Combine(outputDirectory, "process-tree-summary.json");
        if (File.Exists(eventsPath) || File.Exists(summaryPath)) throw new IOException("process_monitor_output_exists");

        HashSet<int> tracked = new HashSet<int>();
        tracked.Add(rootProcessId);
        HashSet<int> recorded = new HashSet<int>();
        List<Entry> unexpected = new List<Entry>();
        Stopwatch elapsed = Stopwatch.StartNew();
        long rootAbsentSince = -1;
        bool rootObserved = false;
        int snapshotCount = 0;

        using (StreamWriter eventsWriter = new StreamWriter(eventsPath, false, new UTF8Encoding(false)))
        {
            while (true)
            {
                List<Entry> entries = Snapshot();
                snapshotCount++;
                bool rootPresent = entries.Exists(delegate(Entry item) { return item.Id == rootProcessId; });
                if (rootPresent) rootObserved = true;

                bool changed;
                do
                {
                    changed = false;
                    foreach (Entry entry in entries)
                    {
                        if (!tracked.Contains(entry.ParentId) || tracked.Contains(entry.Id)) continue;
                        tracked.Add(entry.Id);
                        changed = true;
                        if (recorded.Add(entry.Id))
                        {
                            eventsWriter.WriteLine(
                                "{\"observed_at_utc\":\"" + DateTime.UtcNow.ToString("o") +
                                "\",\"process_id\":" + entry.Id +
                                ",\"parent_process_id\":" + entry.ParentId +
                                ",\"process_name\":\"" + Escape(entry.Name) + "\"}");
                            eventsWriter.Flush();
                            if (IsUnexpectedShell(entry.Name)) unexpected.Add(entry);
                        }
                    }
                }
                while (changed);

                if (durationMilliseconds > 0 && elapsed.ElapsedMilliseconds >= durationMilliseconds) break;
                if (rootObserved && !rootPresent)
                {
                    if (rootAbsentSince < 0) rootAbsentSince = elapsed.ElapsedMilliseconds;
                    if (elapsed.ElapsedMilliseconds - rootAbsentSince >= 2000) break;
                }
                else { rootAbsentSince = -1; }
                Thread.Sleep(25);
            }
        }

        StringBuilder unexpectedJson = new StringBuilder();
        for (int index = 0; index < unexpected.Count; index++)
        {
            if (index > 0) unexpectedJson.Append(',');
            Entry item = unexpected[index];
            unexpectedJson.Append("{\"process_id\":").Append(item.Id)
                .Append(",\"parent_process_id\":").Append(item.ParentId)
                .Append(",\"process_name\":\"").Append(Escape(item.Name)).Append("\"}");
        }
        string summary = "{\"schema\":\"screenwise.gui-process-tree.v1\",\"sampler_available\":true" +
            ",\"poll_interval_ms\":25,\"root_observed\":" + rootObserved.ToString().ToLowerInvariant() +
            ",\"snapshot_count\":" + snapshotCount +
            ",\"descendant_process_start_count\":" + recorded.Count +
            ",\"unexpected_shell_process_start_count\":" + unexpected.Count +
            ",\"unexpected_shell_process_starts\":[" + unexpectedJson + "]}";
        string temporary = summaryPath + ".tmp-" + Guid.NewGuid().ToString("N");
        File.WriteAllText(temporary, summary, new UTF8Encoding(false));
        File.Move(temporary, summaryPath);
        return 0;
    }

    internal static int Main(string[] args)
    {
        try
        {
            if (args.Length > 0 && args[0] == "--resources") return RunResources(args);
            if (args.Length == 1 && args[0] == "--resource-self-test") return ResourceSelfTest();
            if (args.Length == 1 && args[0] == "--self-test")
                return IsUnexpectedShell("PowerShell.exe") && IsUnexpectedShell("setx.exe") &&
                    IsUnexpectedShell("where.exe") && !IsUnexpectedShell("screenpipe-app.exe") ? 0 : 3;
            int rootProcessId = 0;
            int durationMilliseconds = 0;
            string outputDirectory = null;
            for (int index = 0; index < args.Length; index++)
            {
                if (args[index] == "--root-pid" && index + 1 < args.Length) rootProcessId = Int32.Parse(args[++index]);
                else if (args[index] == "--out-dir" && index + 1 < args.Length) outputDirectory = args[++index];
                else if (args[index] == "--duration-ms" && index + 1 < args.Length) durationMilliseconds = Int32.Parse(args[++index]);
                else throw new ArgumentException("unknown_or_incomplete_option");
            }
            return Run(rootProcessId, outputDirectory, durationMilliseconds);
        }
        catch (Exception error)
        {
            // Fixed type/code only; exception messages can contain private paths.
            Console.Error.WriteLine("process_monitor_failed:" + error.GetType().Name);
            System.ComponentModel.Win32Exception native = error as System.ComponentModel.Win32Exception;
            if (native != null) Console.Error.WriteLine("native_error_code:" + native.NativeErrorCode);
            return 2;
        }
    }
}
