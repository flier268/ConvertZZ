using ConvertZZ.Moudle;
using Flier.Toolbox.Text;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

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
        /// 使用OS的kernel.dll做為簡繁轉換工具，只要有裝OS就可以使用，不用額外引用dll，但只能做逐字轉換，無法進行詞意的轉換 
        /// <para>所以無法將電腦轉成計算機</para> 
        /// </summary> 
        [DllImport("kernel32", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern int LCMapStringEx(int Locale, int dwMapFlags, string lpSrcStr, int cchSrc, [Out] string lpDestStr, int cchDest, int lpVersionInformation = 0, int lpReserved = 0, int sortHandle = 0);

        /// <summary> 
        /// 繁體轉簡體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public string ToSimplified(string pSource)
        {
            pSource = FR_Reserved.ReplaceAll(pSource);
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapStringEx(LOCALE_SYSTEM_DEFAULT, LCMAP_SIMPLIFIED_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            tTarget = FRRevert_Reserved.ReplaceAll(tTarget);
            return tTarget;
        }

        /// <summary> 
        /// 簡體轉繁體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public string ToTraditional(string pSource)
        {
            pSource = FR_Reserved.ReplaceAll(pSource);
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapStringEx(LOCALE_SYSTEM_DEFAULT, LCMAP_TRADITIONAL_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            tTarget = FRRevert_Reserved.ReplaceAll(tTarget);
            return tTarget;
        }

        #endregion OS的轉換

        internal List<DictionaryFile_Helper.Line> Lines { get; set; } = new List<DictionaryFile_Helper.Line>();
        FastReplace FR = null, FRRevert = null;
        Dictionary<string, string> ReservedWordTable = new Dictionary<string, string>();
        Dictionary<string, string> ReservedWordTable_Revert = new Dictionary<string, string>();
        public FastReplace FR_Reserved = new FastReplace(new Dictionary<string, string>()), FRRevert_Reserved = new FastReplace(new Dictionary<string, string>());
        public ChineseConverter()
        {
        }
        public async Task Load(string fileName)
        {
            Lines.Clear();
            Lines.AddRange(await DictionaryFile_Helper.Load(fileName));
            Reload();
        }
        public void Reload()
        {
            var lines = Lines.ToLookup(x => x.SimplifiedChinese).Select(coll => coll.First()).ToList();
            FR = new FastReplace(lines.Where(x => x.Enable).OrderByDescending(x => x.SimplifiedChinese_Priority).ThenByDescending(x => x.SimplifiedChinese.Length).ToDictionary(x => x.SimplifiedChinese, x => x.TraditionalChinese));

            lines = Lines.ToLookup(x => x.TraditionalChinese).Select(coll => coll.First()).ToList();
            FRRevert = new FastReplace(lines.Where(x => x.Enable).OrderByDescending(x => x.TraditionalChinese_Priority).ThenByDescending(x => x.TraditionalChinese.Length).ToDictionary(x => x.TraditionalChinese, x => x.SimplifiedChinese));
            ReservedWordTable.Clear();
            ReservedWordTable_Revert.Clear();
            lines.Where(x => x.Enable).Where(x => x.SimplifiedChinese_Priority == 9999 && x.TraditionalChinese_Priority == 9999 && x.SimplifiedChinese == x.TraditionalChinese).ToList().ForEach(x => InsertTo_ReservedWordTable(x.SimplifiedChinese));
            FR_Reserved = new FastReplace(ReservedWordTable);
            FRRevert_Reserved = new FastReplace(ReservedWordTable_Revert);
        }
        /// <summary>
        /// 
        /// </summary>
        /// <param name="input"></param>
        /// <param name="C2T">True:簡體轉繁體  False:繁體轉簡體</param>
        /// <returns></returns>
        public string Convert(string input, bool C2T)
        {
            //這個方法最快
            if (C2T)
                return FR.ReplaceAll(input);
            else
                return FRRevert.ReplaceAll(input);
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
        private void InsertTo_ReservedWordTable(string str)
        {
            string temp;
            do
            {
                temp = $"❂{Guid.NewGuid().ToString()}❂";
            }
            while (ReservedWordTable.ContainsKey(temp));
            ReservedWordTable.Add(str, temp);
            ReservedWordTable_Revert.Add(temp, str);
        }
    }
}
