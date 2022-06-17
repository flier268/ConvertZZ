using System.Diagnostics;

namespace ConvertZZ.Core.Models
{
    public class PRC
    {
        public int PID { get; set; }
        public int Port { get; set; }
        public string? Protocol { get; set; }
        public Process GetProcess => Process.GetProcessById(PID);
    }
}