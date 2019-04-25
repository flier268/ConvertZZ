using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ.Moudle
{
    public class DictionaryFile_Helper
    {
        public static async Task<List<Line>> Load(string CSV_Filename)
        {
            var temp = new List<Line>();
            if (!File.Exists(CSV_Filename))
                throw new Exception($"File \"{CSV_Filename}\" not exist!");
            try
            {
                using (FileStream fileStream = new FileStream(CSV_Filename, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
                {
                    using (StreamReader streamReader = new StreamReader(fileStream, Encoding.UTF8))
                    {
                        string s = await streamReader.ReadToEndAsync();
                        var array_all = s.Split(new string[] { "\r\n" }, StringSplitOptions.RemoveEmptyEntries);
                        foreach (var str in array_all)
                        {
                            var array_line = str.Split('\t').ToList();
                            if (array_line.Count == 6)
                            {
                                array_line.ForEach(x =>
                                {
                                    if (x.StartsWith("\t"))
                                        x = x.Substring(1, x.Length - 2).Replace("\"\"", "\"");
                                });
                                temp.Add(new Line() { Enable = array_line[0] == "True" ? true : false, Type = array_line[1], SimplifiedChinese = array_line[2], SimplifiedChinese_Priority = int.Parse(array_line[3]), TraditionalChinese = array_line[4], TraditionalChinese_Priority = int.Parse(array_line[5]) });
                            }
                        }
                    }
                }
            }
            catch (Exception e) { throw e; }
            return temp;
        }

        public static async Task Save(string CSV_Filename, List<Line> lines)
        {
            try
            {
                using (StreamWriter streamWriter = new StreamWriter(CSV_Filename, false, Encoding.UTF8))
                {
                    streamWriter.AutoFlush = false;
                    Line line = new Line();
                    lines.ForEach(x =>
                    {
                        Line temp = x.Clone();
                        if (temp.SimplifiedChinese_Length + temp.TraditionalChinese_Length > 0)
                        {
                            if (temp.Type.Contains("\""))
                                temp.Type = $"\"{temp.Type.Replace("\"", "\"\"")}\"";
                            if (temp.SimplifiedChinese.Contains("\""))
                                temp.SimplifiedChinese = $"\"{temp.SimplifiedChinese.Replace("\"", "\"\"")}\"";
                            if (temp.TraditionalChinese.Contains("\""))
                                temp.TraditionalChinese = $"\"{temp.TraditionalChinese.Replace("\"", "\"\"")}\"";
                            streamWriter.WriteLine($"{temp.Enable}\t{temp.Type}\t{temp.SimplifiedChinese}\t{temp.SimplifiedChinese_Priority}\t{temp.TraditionalChinese}\t{temp.TraditionalChinese_Priority}");
                        }
                    });
                    await streamWriter.FlushAsync();
                }
            }
            catch (Exception e) { throw e; }
        }

        public class Line
        {
            public Line Clone()
            {
                Line temp = new Line();
                temp.Enable = Enable;
                temp.Type = Type;
                temp.TraditionalChinese = TraditionalChinese;
                temp.SimplifiedChinese = SimplifiedChinese;
                temp.SimplifiedChinese_Priority = SimplifiedChinese_Priority;
                temp.TraditionalChinese_Priority = TraditionalChinese_Priority;
                return temp;
            }
            public bool Enable { get; set; }
            public string SimplifiedChinese { get; set; } = "";
            public int SimplifiedChinese_Length { get => SimplifiedChinese.Length; }
            public int SimplifiedChinese_Priority { get; set; }
            public string TraditionalChinese { get; set; } = "";
            public int TraditionalChinese_Length { get => TraditionalChinese.Length; }
            public int TraditionalChinese_Priority { get; set; }
            public string Type { get; set; } = "Default";
        }
    }
}