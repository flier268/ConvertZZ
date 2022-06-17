using System.Diagnostics;

namespace ConvertZZ.Core
{
    public class StopWatchTask
    {
        public static (T result, long timeCost) Run<T>(Func<T> action)
        {
            Stopwatch watch = Stopwatch.StartNew();
            T result = action();
            watch.Stop();
            return (result, watch.ElapsedMilliseconds);
        }

        public static async Task<(T result, long timeCost)> RunAsync<T>(Func<Task<T>> action)
        {
            Stopwatch watch = Stopwatch.StartNew();
            watch.Start();
            T result = await action();
            watch.Stop();
            return (result, watch.ElapsedMilliseconds);
        }
    }
}