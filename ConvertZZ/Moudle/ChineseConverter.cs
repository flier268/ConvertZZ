using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using static ConvertZZ.FastReplace;

namespace ConvertZZ
{
    public class ChineseConverter
    {
        #region OS的轉換
        internal const int LOCALE_SYSTEM_DEFAULT = 0x0800;
        internal const int LCMAP_SIMPLIFIED_CHINESE = 0x02000000;
        internal const int LCMAP_TRADITIONAL_CHINESE = 0x04000000;

        /// <summary> 
        /// 使用OS的kernel.dll做為簡繁轉換工具，只要有裝OS就可以使用，不用額外引用dll，但只能做逐字轉換，無法進行詞意的轉換 
        /// <para>所以無法將電腦轉成計算機</para> 
        /// </summary> 
        [DllImport("kernel32", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern int LCMapString(int Locale, int dwMapFlags, string lpSrcStr, int cchSrc, [Out] string lpDestStr, int cchDest);

        /// <summary> 
        /// 繁體轉簡體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public static string ToSimplified(string pSource)
        {
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapString(LOCALE_SYSTEM_DEFAULT, LCMAP_SIMPLIFIED_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            return tTarget;
        }

        /// <summary> 
        /// 簡體轉繁體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public static string ToTraditional(string pSource)
        {
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapString(LOCALE_SYSTEM_DEFAULT, LCMAP_TRADITIONAL_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            return tTarget;
        }

        #endregion OS的轉換

        private SortedDictionary<string, string> _dictionary;
        private bool _hasError;
        private StringBuilder _logs;
        FastReplace FR = new FastReplace();
        public ChineseConverter()
        {
            var cmp = new WordMappingComparer();
            _dictionary = new SortedDictionary<string, string>(cmp);
            _logs = new StringBuilder();
            _hasError = false;
        }
        public void ClearLogs()
        {
            _logs.Clear();
            _hasError = false;
        }
        public void Load(string fileName)
        {
            using (var reader = new StreamReader(fileName, Encoding.UTF8))
            {               
                string line = reader.ReadLine();
                while (line != null)
                {
                    this.Add(line);
                    line = reader.ReadLine();
                }
            }

        }

        public void Load(string[] fileNames)
        {
            foreach (string fname in fileNames)
            {
                Load(fname);
            }
        }
        public void ReloadFastReplaceDic()
        {
            FR = new FastReplace(_dictionary);
        }

        public ChineseConverter Add(string sourceWord, string targetWord)
        {
            if (sourceWord == targetWord)
                return this;
            // Skip duplicated words.
            if (_dictionary.ContainsKey(sourceWord))
            {
                _hasError = true;
                _logs.AppendLine(String.Format("警告: '{0}={1}' 的來源字串重複定義, 故忽略此項。", sourceWord, targetWord));
                return this;
            }
            _dictionary.Add(sourceWord, targetWord);
            return this;
        }

        public ChineseConverter Add(string mapping)
        {
            if (!String.IsNullOrWhiteSpace(mapping) && !mapping.StartsWith(";"))
            {
                Regex r = new Regex("(.*?),(.*)");
                var m = r.Match(mapping);
                if (m.Success)
                {
                    this.Add(m.Groups[1].ToString(), m.Groups[2].ToString().Split(' ')[0].ToString());
                }
            }
            return this;
        }

        public ChineseConverter Add(IDictionary<string, string> dict)
        {
            foreach (var key in dict.Keys)
            {
                Add(key, dict[key]);
            }
            return this;
        }

        public string Convert(string input)
        {
            //這個方法最快
            return FR.ReplaceAll(input);
            /* 第二快
            foreach (var temp in _dictionary)
            {
                input = input.Replace(temp.Key, temp.Value);
            }
            return input;*/
            /* 最慢
            StringBuilder sb = new StringBuilder(input);
            foreach(var temp in _dictionary)
            {
                sb.Replace(temp.Key, temp.Value);                    
            }
            return input;*/
        }

        public void DumpKeys()
        {
            foreach (var key in _dictionary.Keys)
            {
                Console.WriteLine(key);
            }
        }

        public bool HasError
        {
            get
            {
                return _hasError;
            }
        }

        public string Logs
        {
            get
            {
                return _logs.ToString();
            }
        }
    }

}
