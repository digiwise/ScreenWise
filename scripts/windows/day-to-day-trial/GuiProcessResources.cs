// Opt-in, content-free process-tree resource sampling. No recorder/API access.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text;
using System.Threading;

internal static partial class GuiProcessTreeMonitor
{
    private sealed class ResourceReading
    {
        internal int Id;
        internal int ParentId;
        internal long Started;
        internal long CpuTicks;
        internal long ClockTicks;
        internal long WorkingSet;
        internal long PrivateBytes;
    }

    private static bool SameProcess(ResourceReading previous, ResourceReading current)
    {
        return previous.Id == current.Id && previous.Started == current.Started;
    }

    private static double? CpuCores(ResourceReading previous, ResourceReading current)
    {
        if (previous == null || !SameProcess(previous, current) ||
            current.ClockTicks <= previous.ClockTicks || current.CpuTicks < previous.CpuTicks) return null;
        return (current.CpuTicks - previous.CpuTicks) / (double)TimeSpan.TicksPerSecond /
            ((current.ClockTicks - previous.ClockTicks) / (double)Stopwatch.Frequency);
    }

    private static string Number(double? value)
    {
        return value.HasValue ? value.Value.ToString("0.######", CultureInfo.InvariantCulture) : "null";
    }

    private static HashSet<int> SelectResourceTree(Dictionary<int, ResourceReading> readings,
        Dictionary<int, ResourceReading> previous, int rootId, long rootStarted)
    {
        HashSet<int> selected = new HashSet<int>();
        foreach (ResourceReading item in readings.Values)
        {
            ResourceReading old;
            if ((item.Id == rootId && item.Started == rootStarted) ||
                (previous.TryGetValue(item.Id, out old) && SameProcess(old, item))) selected.Add(item.Id);
        }
        bool changed;
        do
        {
            changed = false;
            foreach (ResourceReading item in readings.Values)
            {
                ResourceReading parent;
                if (selected.Contains(item.Id) || !selected.Contains(item.ParentId) ||
                    !readings.TryGetValue(item.ParentId, out parent) || parent.Started > item.Started) continue;
                selected.Add(item.Id);
                changed = true;
            }
        } while (changed);
        return selected;
    }

    private static int RunResources(string[] args)
    {
        int rootId = 0, duration = 0, interval = 5000;
        string directory = null;
        for (int index = 1; index < args.Length; index++)
        {
            if (index + 1 >= args.Length) throw new ArgumentException("missing_resource_option");
            string option = args[index++], value = args[index];
            if (option == "--root-pid") rootId = Int32.Parse(value);
            else if (option == "--out-dir") directory = value;
            else if (option == "--duration-ms") duration = Int32.Parse(value);
            else if (option == "--interval-ms") interval = Int32.Parse(value);
            else throw new ArgumentException("unknown_resource_option");
        }
        if (rootId <= 0 || duration < 1000 || duration > 86400000 || interval < 1000 || interval > 60000 ||
            String.IsNullOrWhiteSpace(directory) || !Path.IsPathRooted(directory) || !Directory.Exists(directory))
            throw new ArgumentException("invalid_resource_options");
        long rootStarted;
        using (Process root = Process.GetProcessById(rootId)) rootStarted = root.StartTime.ToUniversalTime().Ticks;
        string output = Path.Combine(Path.GetFullPath(directory), "process-resources.jsonl");
        Dictionary<int, ResourceReading> previous = new Dictionary<int, ResourceReading>();
        Stopwatch elapsed = Stopwatch.StartNew();
        // CreateNew prevents overwriting private evidence. Only the prior sample is retained.
        using (StreamWriter writer = new StreamWriter(new FileStream(output, FileMode.CreateNew, FileAccess.Write), new UTF8Encoding(false)))
        {
            while (true)
            {
                long sampleStarted = elapsed.ElapsedMilliseconds;
                List<Entry> entries = Snapshot();
                Dictionary<int, Entry> candidates = new Dictionary<int, Entry>();
                foreach (Entry entry in entries) candidates[entry.Id] = entry;
                // Narrow before opening process handles; do not read unrelated applications' counters.
                HashSet<int> potential = new HashSet<int>();
                potential.Add(rootId);
                foreach (int id in previous.Keys) potential.Add(id);
                bool changed;
                do
                {
                    changed = false;
                    foreach (Entry entry in entries)
                        if (potential.Contains(entry.ParentId) && potential.Add(entry.Id)) changed = true;
                } while (changed);

                int unavailable = 0;
                bool truncated = false;
                Dictionary<int, ResourceReading> readings = new Dictionary<int, ResourceReading>();
                Dictionary<int, Process> handles = new Dictionary<int, Process>();
                HashSet<int> selected;
                try
                {
                    // Identity first: a reused root or stale parent PID can add an
                    // unrelated process here, so only its start time is read.
                    foreach (int id in potential)
                    {
                        Entry entry;
                        if (!candidates.TryGetValue(id, out entry)) continue;
                        if (readings.Count >= 4096) { truncated = true; break; }
                        Process process = null;
                        try
                        {
                            process = Process.GetProcessById(id);
                            readings[id] = new ResourceReading { Id = id, ParentId = entry.ParentId,
                                Started = process.StartTime.ToUniversalTime().Ticks };
                            handles[id] = process;
                            process = null;
                        }
                        catch (ArgumentException) { unavailable++; }
                        catch (InvalidOperationException) { unavailable++; }
                        catch (System.ComponentModel.Win32Exception) { unavailable++; }
                        finally { if (process != null) process.Dispose(); }
                    }
                    // Counters only for processes verified as part of the tree.
                    HashSet<int> verified = SelectResourceTree(readings, previous, rootId, rootStarted);
                    foreach (int id in verified)
                    {
                        try
                        {
                            Process process = handles[id];
                            ResourceReading item = readings[id];
                            item.CpuTicks = process.TotalProcessorTime.Ticks;
                            item.ClockTicks = Stopwatch.GetTimestamp();
                            item.WorkingSet = process.WorkingSet64;
                            item.PrivateBytes = process.PrivateMemorySize64;
                        }
                        catch (InvalidOperationException) { readings.Remove(id); unavailable++; }
                        catch (System.ComponentModel.Win32Exception) { readings.Remove(id); unavailable++; }
                    }
                    verified.IntersectWith(readings.Keys);
                    selected = verified;
                }
                finally
                {
                    foreach (Process process in handles.Values) process.Dispose();
                }
                Dictionary<int, ResourceReading> current = new Dictionary<int, ResourceReading>();
                StringBuilder processes = new StringBuilder();
                long workingSet = 0, privateBytes = 0;
                double cores = 0;
                int cpuCount = 0;
                foreach (int id in selected)
                {
                    ResourceReading item = readings[id], old;
                    previous.TryGetValue(id, out old);
                    double? cpu = CpuCores(old, item);
                    if (cpu.HasValue) { cores += cpu.Value; cpuCount++; }
                    workingSet += item.WorkingSet;
                    privateBytes += item.PrivateBytes;
                    if (processes.Length > 0) processes.Append(',');
                    processes.Append("{\"pid\":").Append(id).Append(",\"started_utc_ticks\":").Append(item.Started)
                        .Append(",\"cpu_cores\":").Append(Number(cpu))
                        .Append(",\"working_set_bytes\":").Append(item.WorkingSet)
                        .Append(",\"private_bytes\":").Append(item.PrivateBytes).Append('}');
                    current[id] = item;
                }
                writer.WriteLine("{\"schema\":\"screenwise.process-resources.v1\",\"observed_at_utc\":\"" + DateTime.UtcNow.ToString("o") +
                    "\",\"elapsed_ms\":" + elapsed.ElapsedMilliseconds + ",\"interval_ms\":" + interval +
                    ",\"root_pid\":" + rootId + ",\"root_started_utc_ticks\":" + rootStarted +
                    ",\"logical_processors\":" + Environment.ProcessorCount + ",\"root_present\":" + selected.Contains(rootId).ToString().ToLowerInvariant() +
                    ",\"unavailable_count\":" + unavailable + ",\"truncated\":" + truncated.ToString().ToLowerInvariant() +
                    ",\"process_count\":" + current.Count + ",\"cpu_sample_count\":" + cpuCount +
                    ",\"cpu_cores_sampled\":" + Number(cpuCount > 0 ? (double?)cores : null) +
                    ",\"cpu_machine_percent_sampled\":" + Number(cpuCount > 0 ? (double?)(100 * cores / Environment.ProcessorCount) : null) +
                    ",\"working_set_bytes_sum\":" + workingSet + ",\"private_bytes_sum\":" + privateBytes +
                    ",\"sampling_duration_ms\":" + (elapsed.ElapsedMilliseconds - sampleStarted) +
                    ",\"processes\":[" + processes + "]}");
                writer.Flush();
                previous = current;
                if (elapsed.ElapsedMilliseconds >= duration) break;
                Thread.Sleep((int)Math.Max(1, Math.Min(duration - elapsed.ElapsedMilliseconds,
                    interval - (elapsed.ElapsedMilliseconds - sampleStarted))));
            }
        }
        return 0;
    }

    private static int ResourceSelfTest()
    {
        ResourceReading old = new ResourceReading { Id = 1, Started = 10, ClockTicks = 1, CpuTicks = 0 };
        ResourceReading now = new ResourceReading { Id = 1, Started = 10, ClockTicks = 1 + Stopwatch.Frequency, CpuTicks = TimeSpan.TicksPerSecond * 2 };
        if (CpuCores(old, now) != 2 || CpuCores(null, now).HasValue) return 3;
        now.Started = 11;
        if (CpuCores(old, now).HasValue) return 3;
        now.Started = 10;
        now.CpuTicks = -1;
        if (CpuCores(old, now).HasValue) return 3;
        Dictionary<int, ResourceReading> readings = new Dictionary<int, ResourceReading>();
        readings[1] = old;
        readings[2] = new ResourceReading { Id = 2, ParentId = 1, Started = 20 };
        readings[3] = new ResourceReading { Id = 3, ParentId = 1, Started = 5 };
        Dictionary<int, ResourceReading> previous = new Dictionary<int, ResourceReading>();
        HashSet<int> tree = SelectResourceTree(readings, previous, 1, 10);
        if (!tree.Contains(2) || tree.Contains(3)) return 3;
        previous[2] = readings[2];
        readings[1] = new ResourceReading { Id = 1, Started = 30 };
        tree = SelectResourceTree(readings, previous, 1, 10);
        if (tree.Contains(1) || !tree.Contains(2)) return 3;
        readings[2] = new ResourceReading { Id = 2, Started = 40 };
        if (SelectResourceTree(readings, previous, 1, 10).Count != 0) return 3;
        return 0;
    }
}
